use crate::images::{FileBuilder, FileSource, ImageManager, ImageRef, LayerBuilder};
use crate::policy::{load_policy, Policy};
use anyhow::{anyhow, Result};
use bollard::container::{Config, LogOutput, LogsOptions, WaitContainerOptions};
use bollard::models::{HostConfig, Mount, MountTypeEnum};
use bollard::Docker;
use futures_util::stream::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs::{canonicalize, rename};
use tokio::io::{stderr, AsyncWriteExt};
use uuid::Uuid;

const EIF_FILE_NAME: &str = "application.eif";
const ENCLAVE_POLICY_PATH: &str = "/etc/enclaver/policy.yaml";
const ENCLAVE_POLICY_CHOWN: &str = "100:100";

const RELEASE_EIF_PATH: &str = "/enclave/application.eif";
const RELEASE_POLICY_PATH: &str = "/enclave/policy.yaml";

const NITRO_CLI_IMAGE: &str = "us-docker.pkg.dev/edgebit-containers/containers/nitro-cli";
const RELEASE_BASE_IMAGE: &str =
    "us-docker.pkg.dev/edgebit-containers/containers/enclaver-wrapper-base";

pub struct EnclaveArtifactBuilder {
    docker: Arc<Docker>,
    image_manager: ImageManager,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct EIFInfo {
    #[serde(rename = "Measurements")]
    measurements: EIFMeasurements,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct EIFMeasurements {
    #[serde(rename = "PCR0")]
    pcr0: String,

    #[serde(rename = "PCR1")]
    pcr1: String,

    #[serde(rename = "PCR2")]
    pcr2: String,
}

impl EnclaveArtifactBuilder {
    pub fn new() -> Result<Self> {
        let docker_client = Arc::new(Docker::connect_with_local_defaults()?);

        Ok(Self {
            docker: docker_client.clone(),
            image_manager: ImageManager::new_with_docker(docker_client.clone())?,
        })
    }

    /// Build a release image based on the referenced policy.
    pub async fn build_release(&self, policy_path: &str) -> Result<(EIFInfo, ImageRef)> {
        let (_policy, build_dir, eif_info) = self.common_build(policy_path).await?;
        let eif_path = build_dir.path().join(EIF_FILE_NAME);
        let release_img = self.package_eif(eif_path, policy_path).await?;

        Ok((eif_info, release_img))
    }

    /// Build an EIF, as would be included in a release image, based on the referenced policy.
    pub async fn build_eif_only(
        &self,
        policy_path: &str,
        dst_path: &str,
    ) -> Result<(EIFInfo, PathBuf)> {
        let (_policy, build_dir, eif_info) = self.common_build(policy_path).await?;
        let eif_path = build_dir.path().join(EIF_FILE_NAME);
        rename(&eif_path, dst_path).await?;

        Ok((eif_info, canonicalize(dst_path).await?))
    }

    /// Load the referenced policy, amend the image it references to match what we expect in
    /// an enclave, then convert the resulting image to an EIF.
    pub async fn common_build(&self, policy_path: &str) -> Result<(Policy, TempDir, EIFInfo)> {
        let policy = load_policy(policy_path).await?;
        let source_img = self.image_manager.image(&policy.image).await?;
        let amended_img = self.amend_source_image(&source_img, policy_path).await?;

        let build_dir = TempDir::new()?;

        let eif_info = self
            .image_to_eif(&amended_img, &build_dir, EIF_FILE_NAME)
            .await?;

        Ok((policy, build_dir, eif_info))
    }

    /// Amend a source image by adding one or more layers containing the files we expect
    /// to have within the enclave.
    async fn amend_source_image(
        &self,
        source_img: &ImageRef,
        policy_path: &str,
    ) -> Result<ImageRef> {
        let amended_image = self
            .image_manager
            .append_layer(
                &source_img,
                LayerBuilder::new().append_file(FileBuilder {
                    path: PathBuf::from(ENCLAVE_POLICY_PATH),
                    source: FileSource::Local {
                        path: PathBuf::from(policy_path),
                    },
                    chown: ENCLAVE_POLICY_CHOWN.to_string(),
                }),
            )
            .await?;

        Ok(amended_image)
    }

    /// Convert an EIF file into a release OCI image.
    ///
    /// TODO: this currently is incomplete; file permissions are wrong, the base image
    /// doesn't match our current requirements, and the exact intended format is still
    /// TBD.
    async fn package_eif(&self, eif_path: PathBuf, policy_path: &str) -> Result<ImageRef> {
        let base_img = self.image_manager.image(RELEASE_BASE_IMAGE).await?;

        let packaged_img = self
            .image_manager
            .append_layer(
                &base_img,
                LayerBuilder::new()
                    .append_file(FileBuilder {
                        path: PathBuf::from(RELEASE_POLICY_PATH),
                        source: FileSource::Local {
                            path: PathBuf::from(policy_path),
                        },
                        chown: ENCLAVE_POLICY_CHOWN.to_string(),
                    })
                    .append_file(FileBuilder {
                        path: PathBuf::from(RELEASE_EIF_PATH),
                        source: FileSource::Local { path: eif_path },
                        chown: ENCLAVE_POLICY_CHOWN.to_string(),
                    }),
            )
            .await?;

        Ok(packaged_img)
    }

    /// Convert the referenced image to an EIF file, which will be deposited into `build_dir`
    /// using the file name `eif_name`.
    ///
    /// This operates by mounting the build dir into a docker container, and invoking `nitro-cli build-enclave`
    /// inside that container.
    async fn image_to_eif(
        &self,
        source_img: &ImageRef,
        build_dir: &TempDir,
        eif_name: &str,
    ) -> Result<EIFInfo> {
        let mut stderr = stderr();

        let build_dir_path = build_dir.path().to_str().unwrap();

        // There is currently no way to point nitro-cli to a local image ID; it insists
        // on attempting to pull the image (this may be a bug;. As a workaround, give our image a random
        // tag, and pass that.
        let img_tag = Uuid::new_v4().to_string();
        self.image_manager.tag_image(&source_img, &img_tag).await?;

        let build_container_id = self
            .docker
            .create_container::<&str, &str>(
                None,
                Config {
                    image: Some(NITRO_CLI_IMAGE),
                    cmd: Some(vec![
                        "build-enclave",
                        "--docker-uri",
                        &img_tag,
                        "--output-file",
                        eif_name,
                    ]),
                    attach_stderr: Some(true),
                    attach_stdout: Some(true),
                    host_config: Some(HostConfig {
                        mounts: Some(vec![
                            Mount {
                                typ: Some(MountTypeEnum::BIND),
                                source: Some(String::from("/var/run/docker.sock")),
                                target: Some(String::from("/var/run/docker.sock")),
                                ..Default::default()
                            },
                            Mount {
                                typ: Some(MountTypeEnum::BIND),
                                source: Some(build_dir_path.into()),
                                target: Some(String::from("/build")),
                                ..Default::default()
                            },
                        ]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await?
            .id;

        self.docker
            .start_container::<String>(&build_container_id, None)
            .await?;

        // Stream stderr logs to stderr. This is useful when debugging failures, but
        // also provides visual feedback that something is happening when on track
        // to succeed. It is kind of weird for this function to have a side-effect like
        // this; perhaps the EnclaveArtifactBuilder should be passed some kind of logging
        // facility?
        let mut log_stream = self.docker.logs::<String>(
            &build_container_id,
            Some(LogsOptions {
                follow: true,
                stderr: true,
                ..Default::default()
            }),
        );

        while let Some(Ok(LogOutput::StdErr { message })) = log_stream.next().await {
            stderr.write_all(message.as_ref()).await?;
        }

        let status_code = self
            .docker
            .wait_container(&build_container_id, None::<WaitContainerOptions<String>>)
            .try_collect::<Vec<_>>()
            .await?
            .first()
            .ok_or(anyhow!("missing wait response from daemon",))?
            .status_code;

        if status_code != 0 {
            return Err(anyhow!("non-zero exit code from nitro-cli",));
        }

        let mut json_buf = Vec::with_capacity(4096);
        let mut log_stream = self.docker.logs::<String>(
            &build_container_id,
            Some(LogsOptions {
                stdout: true,
                ..Default::default()
            }),
        );

        while let Some(Ok(LogOutput::StdOut { message })) = log_stream.next().await {
            json_buf.extend_from_slice(message.as_ref());
        }

        Ok(serde_json::from_slice(&json_buf)?)
    }
}
