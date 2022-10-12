use crate::images::{FileBuilder, FileSource, ImageManager, ImageRef, LayerBuilder};
use crate::nitro_cli::EIFInfo;
use crate::manifest::{load_manifest, Manifest};
use anyhow::{anyhow, Result};
use bollard::container::{Config, LogOutput, LogsOptions, WaitContainerOptions};
use bollard::image::CreateImageOptions;
use bollard::models::{ContainerConfig, CreateImageInfo, HostConfig, Mount, MountTypeEnum};
use bollard::Docker;
use futures_util::stream::{StreamExt, TryStreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use log::{debug, info};
use tempfile::TempDir;
use tokio::fs::{canonicalize, rename};
use tokio::io::{stderr, AsyncWriteExt};
use uuid::Uuid;
use crate::constants::{ENCLAVE_CONFIG_DIR, CONFIG_FILE_NAME, ENCLAVE_ODYN_PATH};

const EIF_FILE_NAME: &str = "application.eif";

const ENCLAVE_MANIFEST_PERMS: &str = "440";
const ENCLAVE_ODYN_PERMS: &str = "550";
const ENCLAVE_OVERLAY_CHOWN: &str = "0:0";

const RELEASE_BUNDLE_DIR: &str = "/enclave";
const RELEASE_OVERLAY_PERMS: &str = "444";
const RELEASE_OVERLAY_CHOWN: &str = "0:0";

const NITRO_CLI_IMAGE: &str = "us-docker.pkg.dev/edgebit-containers/containers/nitro-cli";
const ODYN_IMAGE: &str = "us-docker.pkg.dev/edgebit-containers/containers/odyn";
const ODYN_IMAGE_BINARY_PATH: &str = "/usr/local/bin/odyn";
const RELEASE_BASE_IMAGE: &str =
    "us-docker.pkg.dev/edgebit-containers/containers/enclaver-wrapper-base";

pub struct EnclaveArtifactBuilder {
    docker: Arc<Docker>,
    image_manager: ImageManager,
}

impl EnclaveArtifactBuilder {
    pub fn new() -> Result<Self> {
        let docker_client = Arc::new(Docker::connect_with_local_defaults()
            .map_err(|e| anyhow!("connecting to docker: {}", e))?);

        Ok(Self {
            docker: docker_client.clone(),
            image_manager: ImageManager::new_with_docker(docker_client)?,
        })
    }

    /// Build a release image based on the referenced manifest.
    pub async fn build_release(&self, manifest_path: &str) -> Result<(EIFInfo, ImageRef, String)> {
        let (manifest, build_dir, eif_info) = self.common_build(manifest_path).await?;
        let eif_path = build_dir.path().join(EIF_FILE_NAME);
        let release_img = self.package_eif(eif_path, manifest_path).await?;

        let release_tag = &manifest.images.target;

        self.image_manager.tag_image(&release_img, release_tag).await?;

        Ok((eif_info, release_img, release_tag.to_string()))
    }

    /// Build an EIF, as would be included in a release image, based on the referenced manifest.
    pub async fn build_eif_only(
        &self,
        manifest_path: &str,
        dst_path: &str,
    ) -> Result<(EIFInfo, PathBuf)> {
        let (_manifest, build_dir, eif_info) = self.common_build(manifest_path).await?;
        let eif_path = build_dir.path().join(EIF_FILE_NAME);
        rename(&eif_path, dst_path).await?;

        Ok((eif_info, canonicalize(dst_path).await?))
    }

    /// Load the referenced manifest, amend the image it references to match what we expect in
    /// an enclave, then convert the resulting image to an EIF.
    pub async fn common_build(&self, manifest_path: &str) -> Result<(Manifest, TempDir, EIFInfo)> {
        let manifest = load_manifest(manifest_path).await?;

        self.analyze_manifest(&manifest);

        let source_img = self.image_manager.image(&manifest.images.source).await?;
        let amended_img = self.amend_source_image(&source_img, manifest_path).await?;

        info!("built intermediate image: {}", amended_img);

        let build_dir = TempDir::new()?;

        let eif_info = self
            .image_to_eif(&amended_img, &build_dir, EIF_FILE_NAME)
            .await?;

        Ok((manifest, build_dir, eif_info))
    }

    /// Amend a source image by adding one or more layers containing the files we expect
    /// to have within the enclave.
    async fn amend_source_image(
        &self,
        source_img: &ImageRef,
        manifest_path: &str,
    ) -> Result<ImageRef> {
        let img_config = self.docker.inspect_image(source_img.to_str()).await?.config;

        // Find the CMD and ENTRYPOINT from the source image. If either was specified in "shell form"
        // Docker seems to convert it to "exec form" as an actual shell invocation, so we can simply
        // ignore that possibility.
        //
        // Since the enclave image cannot take any arguments (which would normally override a CMD),
        // we can simply take everything from CMD and append it to the ENTRYPOINT, then append that
        // whole thing to the odyn invocation.
        // TODO(russell_h): Figure out what happens when a source image specifies env variables.
        let mut cmd = match img_config {
            Some(ContainerConfig {
                cmd: Some(ref cmd), ..
            }) => cmd.clone(),
            _ => vec![],
        };

        let mut entrypoint = match img_config {
            Some(ContainerConfig {
                entrypoint: Some(ref entrypoint),
                ..
            }) => entrypoint.clone(),
            _ => vec![],
        };

        let mut odyn_command = vec![
            String::from(ENCLAVE_ODYN_PATH),
            String::from("--config-dir"),
            String::from("/etc/enclaver"),
            String::from("--"),
        ];

        odyn_command.append(&mut entrypoint);
        odyn_command.append(&mut cmd);

        debug!("appending layer to source image");
        let amended_image = self
            .image_manager
            .append_layer(
                source_img,
                LayerBuilder::new()
                    .append_file(FileBuilder {
                        path: PathBuf::from(ENCLAVE_CONFIG_DIR).join(CONFIG_FILE_NAME),
                        source: FileSource::Local {
                            path: PathBuf::from(manifest_path),
                        },
                        chown: ENCLAVE_OVERLAY_CHOWN.to_string(),
                        chmod: ENCLAVE_MANIFEST_PERMS.into(),
                    })
                    .append_file(FileBuilder {
                        path: PathBuf::from(ENCLAVE_ODYN_PATH),
                        source: FileSource::Image {
                            name: format!("{ODYN_IMAGE}:latest").into(),
                            path: ODYN_IMAGE_BINARY_PATH.into(),
                        },
                        chown: ENCLAVE_OVERLAY_CHOWN.to_string(),
                        chmod: ENCLAVE_ODYN_PERMS.into(),
                    })
                    .set_entrypoint(odyn_command),
            )
            .await?;

        Ok(amended_image)
    }

    /// Convert an EIF file into a release OCI image.
    ///
    /// TODO: this currently is incomplete; file permissions are wrong, the base image
    /// doesn't match our current requirements, and the exact intended format is still
    /// TBD.
    async fn package_eif(&self, eif_path: PathBuf, manifest_path: &str) -> Result<ImageRef> {
        let base_img = self.pull_image(format!("{RELEASE_BASE_IMAGE}:latest").as_str()).await?;

        debug!("packaging EIF file: {}", eif_path.to_string_lossy());

        let packaged_img = self
            .image_manager
            .append_layer(
                &base_img,
                LayerBuilder::new()
                    .append_file(FileBuilder {
                        path: PathBuf::from(RELEASE_BUNDLE_DIR).join(CONFIG_FILE_NAME),
                        source: FileSource::Local {
                            path: PathBuf::from(manifest_path),
                        },
                        chown: RELEASE_OVERLAY_CHOWN.to_string(),
                        chmod: RELEASE_OVERLAY_PERMS.into(),
                    })
                    .append_file(FileBuilder {
                        path: PathBuf::from(RELEASE_BUNDLE_DIR).join(EIF_FILE_NAME),
                        source: FileSource::Local { path: eif_path },
                        chown: RELEASE_OVERLAY_CHOWN.to_string(),
                        chmod: RELEASE_OVERLAY_PERMS.into(),
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
        self.image_manager.tag_image(source_img, &img_tag).await?;

        debug!("tagged intermediate image: {}", img_tag);

        let nitro_cli = self.pull_image(NITRO_CLI_IMAGE).await?;

        let build_container_id = self
            .docker
            .create_container::<&str, &str>(
                None,
                Config {
                    image: Some(nitro_cli.to_str()),
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

        info!("starting nitro-cli build-eif in container: {}", build_container_id);

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
            .ok_or_else(|| anyhow!("missing wait response from daemon",))?
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

    /// Pull an image from a remote registry, if it is not already present, while streaming
    /// output to the terminal.
    async fn pull_image(&self, image_name: &str) -> Result<ImageRef> {
        debug!("fetching image: {}", image_name);
        let mut fetch_stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: image_name,
                ..Default::default()
            }),
            None,
            None,
        );

        while let Some(item) = fetch_stream.next().await {
            let create_image_info = item?;
            if let CreateImageInfo {
                id: Some(id),
                status: Some(status),
                ..
            } = create_image_info
            {
                println!("{}: {}", id, status);
            }
        }

        self.image_manager.image(image_name).await
    }

    fn analyze_manifest(&self, manifest: &Manifest) {
        if manifest.ingress.is_none() {
            info!("no ingress specified in manifest; there will be no way to connect to this enclave");
        }

        if manifest.egress.is_none() {
            info!("no egress specified in manifest; this enclave will have no network access");
        }
    }
}
