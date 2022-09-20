use crate::error::{Error, Result};
use crate::images::{FileBuilder, FileSource, ImageManager, LayerBuilder};
use crate::policy::load_policy;
use bollard::container::{Config, LogOutput, LogsOptions, WaitContainerOptions};
use bollard::models::{HostConfig, Mount, MountTypeEnum};
use bollard::Docker;
use futures_util::stream::{StreamExt, TryStreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{stderr, AsyncWriteExt};
use uuid::Uuid;

pub struct EnclaveArtifactBuilder {
    docker: Arc<Docker>,
    image_manager: ImageManager,
}

impl EnclaveArtifactBuilder {
    pub fn new() -> Result<Self> {
        let docker_client = Arc::new(Docker::connect_with_local_defaults()?);

        Ok(Self {
            docker: docker_client.clone(),
            image_manager: ImageManager::new_with_docker(docker_client.clone())?,
        })
    }

    pub async fn build_artifact(&self, policy_path: &str) -> Result<()> {
        let mut stderr = stderr();
        let policy = load_policy(policy_path).await?;
        let source_img = self.image_manager.image(&policy.image).await?;
        let res_image = self
            .image_manager
            .append_layer(
                &source_img,
                LayerBuilder::new().append_file(FileBuilder {
                    path: PathBuf::from("/etc/enclaver/policy.yaml"),
                    source: FileSource::Local {
                        path: PathBuf::from(policy_path),
                    },
                    chown: "100:100".to_string(),
                }),
            )
            .await?;

        // There is currently no way to point nitro-cli to a local image ID; it insists
        // on attempting to pull the image. As a workaround, give our image a random
        // tag, and pass that.
        let img_tag = Uuid::new_v4().to_string();
        self.image_manager.tag_image(&res_image, &img_tag).await?;

        let tmpdir = TempDir::new()?;
        let tmpdir_str = tmpdir.path().to_str().unwrap();

        let build_container_id = self
            .docker
            .create_container::<&str, &str>(
                None,
                Config {
                    image: Some("us-docker.pkg.dev/edgebit-containers/containers/nitro-cli"),
                    cmd: Some(vec![
                        "build-enclave",
                        "--docker-uri",
                        &img_tag,
                        "--output-file",
                        "application.eif",
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
                                source: Some(tmpdir_str.into()),
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

        // Stream stderr logs to stderr
        let mut log_stream = self.docker.logs::<String>(
            &build_container_id,
            Some(LogsOptions {
                follow: true,
                stderr: true,
                ..Default::default()
            }),
        );

        loop {
            if let Some(Ok(LogOutput::StdErr { message })) = log_stream.next().await {
                stderr.write_all(message.as_ref()).await?;
            } else {
                break;
            }
        }

        let status_code = self
            .docker
            .wait_container(&build_container_id, None::<WaitContainerOptions<String>>)
            .try_collect::<Vec<_>>()
            .await?
            .first()
            .ok_or(Error::InvalidDaemonResponse(String::from(
                "missing wait response from daemon",
            )))?
            .status_code;

        if status_code != 0 {
            return Err(Error::NitroCLIError(String::from(
                "non-zero exit code from nitro-cli",
            )));
        }

        let mut json_buf = Vec::with_capacity(4096);

        let mut logstream = self.docker.logs::<String>(
            &build_container_id,
            Some(LogsOptions {
                stdout: true,
                ..Default::default()
            }),
        );

        loop {
            if let Some(Ok(LogOutput::StdOut { message })) = logstream.next().await {
                json_buf.extend_from_slice(message.as_ref());
            } else {
                break;
            }
        }

        stderr.write_all(&json_buf).await?;

        Ok(())
    }
}
