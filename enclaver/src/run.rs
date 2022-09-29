use crate::nitro_cli::{NitroCLI, RunEnclaveArgs};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Enclave {
    cli: NitroCLI,
    eif_path: PathBuf,
    cpu_count: i32,
    memory_mb: i32,
    enclave_id: Option<String>,
}

#[derive(Debug)]
pub enum EnclaveState {
    None,
    Running(String),
    Stopped(String),
}

impl Enclave {
    pub fn new<P>(eif_path: P, cpu_count: i32, memory_mb: i32) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            cli: NitroCLI::new(),
            eif_path: eif_path.as_ref().to_path_buf(),
            cpu_count,
            memory_mb,
            enclave_id: None,
        }
    }

    pub async fn start(&mut self) -> Result<EnclaveState> {
        if self.enclave_id.is_some() {
            return Err(anyhow!("Enclave already started"));
        }

        let enclave_info = self
            .cli
            .run_enclave(RunEnclaveArgs {
                cpu_count: self.cpu_count,
                memory_mb: self.memory_mb,
                eif_path: self.eif_path.clone(),
                cid: None,
            })
            .await?;

        println!("Enclave: {:#?}", enclave_info);

        self.enclave_id = Some(enclave_info.id.clone());

        Ok(EnclaveState::Running(enclave_info.id))
    }

    pub async fn run_with_debug(&self) -> Result<()> {
        self.cli
            .run_enclave_with_debug(RunEnclaveArgs {
                cpu_count: self.cpu_count,
                memory_mb: self.memory_mb,
                eif_path: self.eif_path.clone(),
                cid: None,
            })
            .await
    }

    pub async fn state(&self) -> Result<EnclaveState> {
        match self.enclave_id {
            Some(ref id) => {
                let exists = self
                    .cli
                    .describe_enclaves()
                    .await?
                    .into_iter()
                    .any(|e| e.id == *id);

                match exists {
                    true => Ok(EnclaveState::Running(id.clone())),
                    false => Ok(EnclaveState::Stopped(id.clone())),
                }
            }
            None => Ok(EnclaveState::None),
        }
    }

    pub async fn stop(&self) -> Result<()> {
        match self.enclave_id {
            Some(ref id) => {
                self.cli.terminate_enclave(id).await?;
                Ok(())
            }
            None => Err(anyhow!("Enclave not started")),
        }
    }

    pub async fn wait(&self) -> Result<()> {
        loop {
            match self.state().await? {
                EnclaveState::Running(_) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                EnclaveState::Stopped(_) => {
                    return Ok(());
                }
                EnclaveState::None => {
                    return Err(anyhow!("Enclave not started"));
                }
            }
        }
    }
}
