use std::path::{Path, PathBuf};
use std::time::{Duration};

use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures_util::stream::{Stream, TryStreamExt};
use tokio_util::codec::{BytesCodec, FramedRead};
use tokio_vsock::VsockStream;
use crate::constants::APP_LOG_PORT;

use crate::nitro_cli::{EnclaveInfo, NitroCLI, RunEnclaveArgs};

const LOG_VSOCK_RETRY_INTERVAL: Duration = Duration::from_millis(250);

pub struct Enclave {
    cli: NitroCLI,
    eif_path: PathBuf,
    cpu_count: i32,
    memory_mb: i32,
    enclave_info: Option<EnclaveInfo>,
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
            enclave_info: None,
        }
    }

    pub async fn start(&mut self) -> Result<EnclaveState> {
        if self.enclave_info.is_some() {
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

        let enclave_id = enclave_info.id.clone();

        self.enclave_info = Some(enclave_info);

        Ok(EnclaveState::Running(enclave_id))
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
        match &self.enclave_info {
            Some(EnclaveInfo { id, .. }) => {
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
        match &self.enclave_info {
            Some(info) => self.cli.terminate_enclave(&info.id).await,
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

    pub async fn wait_logs(&self) -> Result<impl Stream<Item = std::io::Result<Bytes>>> {
        let cid = self
            .enclave_info
            .as_ref()
            .ok_or_else(|| anyhow!("Enclave not started"))?
            .cid;

        // Loop until we manage to connect to the vsock.
        let conn = loop {
            match VsockStream::connect(cid, APP_LOG_PORT).await {
                Ok(conn) => break conn,

                // TODO: improve the polling frequency / backoff / timeout
                Err(_) => {
                    tokio::time::sleep(LOG_VSOCK_RETRY_INTERVAL).await;
                }
            }
        };

        let framed = FramedRead::new(conn, BytesCodec::new())
            .map_ok(|bm| bm.freeze());

        Ok(framed)
    }
}
