use crate::error::{Result};
use crate::images::{FileBuilder, FileSource, ImageManager, LayerBuilder};
use crate::policy::load_policy;
use bollard::container::{Config};
use bollard::models::{HostConfig, Mount, MountTypeEnum};
use bollard::Docker;

use std::path::PathBuf;
use std::rc::Rc;
use tempfile::TempDir;

pub struct EnclaveArtifactBuilder {
    docker: Rc<Docker>,
    image_manager: ImageManager,
}

impl EnclaveArtifactBuilder {
    pub fn new() -> Result<Self> {
        let docker_client = Rc::new(Docker::connect_with_local_defaults()?);

        Ok(Self {
            docker: docker_client.clone(),
            image_manager: ImageManager::new_with_docker(docker_client.clone())?,
        })
    }

    pub async fn build_artifact(&self, policy_path: &str) -> Result<()> {
        let policy = load_policy(policy_path).await?;
        let source_img = self.image_manager.image(&policy.image).await?;
        let res_image = self.image_manager
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

        let tmpdir = TempDir::new()?;
        let tmpdir_str = tmpdir.path().to_str().unwrap();

        let container_res = self
            .docker
            .create_container::<&str, &str>(
                None,
                Config {
                    image: Some("us-docker.pkg.dev/edgebit-containers/containers/nitro-cli"),
                    cmd: Some(vec![
                        "build-enclave",
                        "--docker-uri",
                        res_image.to_str(),
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
            .await?;

        println!("container: {:#?}", container_res);

        Ok(())
    }
}
