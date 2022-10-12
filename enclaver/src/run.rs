use std::path::{Path, PathBuf};
use std::time::{Duration};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use log::{info};
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_vsock::VsockStream;
use crate::constants::{APP_LOG_PORT, HTTP_EGRESS_VSOCK_PORT};
use crate::manifest::{load_manifest, Manifest};

use crate::nitro_cli::{EnclaveInfo, NitroCLI, RunEnclaveArgs};
use crate::proxy::egress_http::HostHttpProxy;
use crate::proxy::ingress::HostProxy;

const LOG_VSOCK_RETRY_INTERVAL: Duration = Duration::from_millis(250);

pub struct Enclave {
    cli: NitroCLI,
    eif_path: PathBuf,
    manifest: Manifest,
    cpu_count: i32,
    memory_mb: i32,
    debug_mode: bool,
    enclave_info: Option<EnclaveInfo>,
    proxy_handles: Vec<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
pub enum EnclaveState {
    None,
    Running(String),
    Stopped(String),
}

impl Enclave {
    pub async fn new<P>(eif_path: P, manifest_path: String, cpu_count: i32, memory_mb: i32, debug_mode: bool) -> Result<Self>
    where
        P: AsRef<Path>,
    {

        Ok(Self {
            cli: NitroCLI::new(),
            eif_path: eif_path.as_ref().to_path_buf(),
            manifest: load_manifest(&manifest_path).await?,
            cpu_count,
            memory_mb,
            debug_mode,
            enclave_info: None,
            proxy_handles: Vec::new(),
        })
    }

    pub async fn start(&mut self) -> Result<EnclaveState> {
        if self.enclave_info.is_some() {
            return Err(anyhow!("Enclave already started"));
        }

        // Start the egress proxy before starting the enclave, to avoid (unlikely) race conditions
        // where something inside the enclave attempts egress before the proxy is ready.
        self.start_egress_proxy().await?;

        info!("starting enclave");
        let enclave_info = self
            .cli
            .run_enclave(RunEnclaveArgs {
                cpu_count: self.cpu_count,
                memory_mb: self.memory_mb,
                eif_path: self.eif_path.clone(),
                cid: None,
                debug_mode: self.debug_mode,
            })
            .await?;

        self.enclave_info = Some(enclave_info.clone());

        print!("started enclave {}", enclave_info.id);

        self.start_odyn_log_stream(enclave_info.cid).await?;

        self.start_ingress_proxies(enclave_info.cid).await?;

        Ok(EnclaveState::Running(enclave_info.id))
    }

    async fn start_ingress_proxies(&mut self, cid: u32) -> Result<()> {
        let ingress = match &self.manifest.ingress {
            Some(ref ingress) => ingress,
            None => {
                info!("no ingress defined, no ingress proxies will be started");
                return Ok(())
            },
        };

        for item in ingress {
            let listen_port = item.listen_port;
            let proxy = HostProxy::bind(listen_port).await?;
            self.proxy_handles.push(tokio::task::spawn(async move {
                proxy.serve(cid, listen_port.into()).await;
            }))
        }

        Ok(())
    }

    async fn start_egress_proxy(&mut self) -> Result<()> {
        // Note: we _could_ start the egress proxy no matter what, but there is no sense in it,
        // and skipping it seems (barely) safer - so we may as well.
        let _ = match &self.manifest.egress {
            Some(egress) => egress,
            None => {
                info!("no egress defined, no egress proxy will be started");
                return Ok(())
            },
        };

        info!("starting egress proxy on vsock port {HTTP_EGRESS_VSOCK_PORT}");
        let proxy = HostHttpProxy::bind(HTTP_EGRESS_VSOCK_PORT)?;
        self.proxy_handles.push(tokio::task::spawn(async move {
            proxy.serve().await;
        }));

        Ok(())
    }

    async fn start_odyn_log_stream(&mut self, cid: u32) -> Result<()> {
        info!("waiting for enclave to boot");
        let conn = loop {
            match VsockStream::connect(cid, APP_LOG_PORT).await {
                Ok(conn) => break conn,

                // TODO: improve the polling frequency / backoff / timeout
                Err(_) => {
                    tokio::time::sleep(LOG_VSOCK_RETRY_INTERVAL).await;
                }
            }
        };

        info!("connected to enclave, starting log stream");

        let mut framed = FramedRead::new(conn, LinesCodec::new_with_max_length(1024 * 4));

        self.proxy_handles.push(tokio::task::spawn(async move {
            while let Some(line_res) = framed.next().await {
                match line_res {
                    Ok(line) => info!(target: "enclave", "{line}"),
                    Err(e) => info!("error reading log stream: {e}"),
                }
            }
        }));

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        match &self.enclave_info {
            Some(info) => self.cli.terminate_enclave(&info.id).await,
            None => Err(anyhow!("Enclave not started")),
        }
    }
}
