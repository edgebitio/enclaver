use crate::constants::{
    APP_LOG_PORT, EIF_FILE_NAME, HTTP_EGRESS_VSOCK_PORT, MANIFEST_FILE_NAME, RELEASE_BUNDLE_DIR,
    STATUS_PORT,
};
use crate::manifest::{load_manifest, Defaults, Manifest};
use crate::utils;
use anyhow::{anyhow, Result};
use futures_util::stream::StreamExt;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs::File;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_vsock::VsockStream;

use crate::nitro_cli::{EnclaveInfo, NitroCLI, RunEnclaveArgs};
use crate::proxy::egress_http::HostHttpProxy;
use crate::proxy::ingress::HostProxy;

const LOG_VSOCK_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const STATUS_VSOCK_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const STATUS_VSOCK_RETRY_LIMIT: i32 = 10;

const DEFAULT_CPU_COUNT: i32 = 2;
const DEFAULT_MEMORY_MB: i32 = 4096;

pub struct EnclaveOpts {
    pub eif_path: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub cpu_count: Option<i32>,
    pub memory_mb: Option<i32>,
    pub debug_mode: bool,
}

pub struct Enclave {
    cli: NitroCLI,
    eif_path: PathBuf,
    manifest: Manifest,
    cpu_count: i32,
    memory_mb: i32,
    debug_mode: bool,
    enclave_info: Option<EnclaveInfo>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Enclave {
    pub async fn new(opts: EnclaveOpts) -> Result<Self> {
        let eif_path = match opts.eif_path {
            Some(eif_path) => eif_path,
            None => PathBuf::from(RELEASE_BUNDLE_DIR).join(EIF_FILE_NAME),
        };

        // Test that the EIF exists
        let _ = File::open(&eif_path)
            .await
            .map_err(|e| anyhow!("failed to open EIF file at {}: {e}", eif_path.display()))?;

        let manifest_path = match opts.manifest_path {
            Some(manifest_path) => manifest_path,
            None => PathBuf::from(RELEASE_BUNDLE_DIR).join(MANIFEST_FILE_NAME),
        };

        let manifest = load_manifest(&manifest_path).await?;

        let cpu_count = match (opts.cpu_count, &manifest.defaults) {
            (Some(cpu_count), _) => cpu_count,
            (
                None,
                Some(Defaults {
                    cpu_count: Some(cpu_count),
                    ..
                }),
            ) => {
                debug!("using cpu_count = {cpu_count} based on defaults from manifest");
                *cpu_count
            }
            _ => {
                debug!("no cpu_count specified, defaulting to {DEFAULT_CPU_COUNT}");
                DEFAULT_CPU_COUNT
            }
        };

        let memory_mb = match (opts.memory_mb, &manifest.defaults) {
            (Some(memory_mb), _) => memory_mb,
            (
                None,
                Some(Defaults {
                    memory_mb: Some(memory_mb),
                    ..
                }),
            ) => {
                debug!("using memory_mb = {memory_mb} based on defaults from manifest");
                *memory_mb
            }
            _ => {
                debug!("no memory_mb specified, defaulting to {DEFAULT_MEMORY_MB}");
                DEFAULT_MEMORY_MB
            }
        };

        Ok(Self {
            cli: NitroCLI::new(),
            eif_path: eif_path.to_path_buf(),
            manifest: load_manifest(&manifest_path).await?,
            cpu_count,
            memory_mb,
            debug_mode: opts.debug_mode,
            enclave_info: None,
            tasks: Vec::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
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

        info!("started enclave {}", enclave_info.id);

        if self.debug_mode {
            self.attach_debug_console(&enclave_info.id).await?;
        }

        self.start_odyn_log_stream(enclave_info.cid).await?;

        self.start_ingress_proxies(enclave_info.cid).await?;

        self.await_completion(enclave_info.cid).await?;

        Ok(())
    }

    async fn start_ingress_proxies(&mut self, cid: u32) -> Result<()> {
        let ingress = match &self.manifest.ingress {
            Some(ref ingress) => ingress,
            None => {
                info!("no ingress defined, no ingress proxies will be started");
                return Ok(());
            }
        };

        for item in ingress {
            let listen_port = item.listen_port;
            let proxy = HostProxy::bind(listen_port).await?;
            self.tasks.push(tokio::task::spawn(async move {
                proxy.serve(cid, listen_port.into()).await;
            }))
        }

        Ok(())
    }

    async fn start_egress_proxy(&mut self) -> Result<()> {
        // Note: we _could_ start the egress proxy no matter what, but there is no sense in it,
        // and skipping it seems (barely) safer - so we may as well.
        if self.manifest.egress.is_none() {
            info!("no egress defined, no egress proxy will be started");
            return Ok(());
        }

        info!("starting egress proxy on vsock port {HTTP_EGRESS_VSOCK_PORT}");
        let proxy = HostHttpProxy::bind(HTTP_EGRESS_VSOCK_PORT)?;
        self.tasks.push(tokio::task::spawn(async move {
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

        self.tasks.push(tokio::task::spawn(async move {
            if let Err(e) = utils::log_lines_from_stream("enclave", conn).await {
                error!("error reading log lines from enclave: {e}");
            }
        }));

        Ok(())
    }

    async fn await_completion(&mut self, cid: u32) -> Result<()> {
        let mut failed_attempts = 0;

        loop {
            let conn = match VsockStream::connect(cid, STATUS_PORT).await {
                Ok(conn) => conn,

                Err(_) => {
                    failed_attempts += 1;
                    if failed_attempts >= STATUS_VSOCK_RETRY_LIMIT {
                        return Err(anyhow!(
                            "failed to connect to enclave status port after {STATUS_VSOCK_RETRY_LIMIT} attempts"
                        ));
                    }
                    tokio::time::sleep(STATUS_VSOCK_RETRY_INTERVAL).await;
                    continue;
                }
            };

            debug!("connected to enclave status port");

            let mut framed = FramedRead::new(conn, LinesCodec::new_with_max_length(1024));

            while let Some(line_res) = framed.next().await {
                let line = match line_res {
                    Ok(line) => line,
                    Err(e) => {
                        error!("error reading from status port: {e}");
                        continue;
                    }
                };

                let status: OdynStatus = match serde_json::from_str(&line) {
                    Ok(status) => status,
                    Err(e) => {
                        error!("error parsing status line: {e}");
                        continue;
                    }
                };

                debug!("enclave status: {status:#?}");

                match status {
                    OdynStatus::Running => {}
                    OdynStatus::Exited { code } => {
                        info!("enclave exited with code {code}");
                        return Ok(());
                    }
                    OdynStatus::Signaled { signal } => {
                        info!("enclave stopped due to signal {signal}");
                        return Ok(());
                    }
                }
            }

            error!("enclave status port closed unexpectedly");
        }
    }

    async fn attach_debug_console(&mut self, enclave_id: &str) -> Result<()> {
        info!("attaching to debug console");

        let stdout = self.cli.console(enclave_id).await?;

        self.tasks.push(tokio::task::spawn(async move {
            if let Err(e) = utils::log_lines_from_stream("nitro-cli::console", stdout).await {
                error!("error reading log lines from debug console: {e}");
            }
        }));

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        if let Some(enclave_info) = &self.enclave_info {
            info!("stopping enclave {}", enclave_info.id);
            self.cli.terminate_enclave(&enclave_info.id).await?;
        } else {
            debug!("no enclave to stop");
        }

        // TODO: stop other tasks

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status")]
enum OdynStatus {
    #[serde(rename = "running")]
    Running,

    #[serde(rename = "exited")]
    Exited { code: i32 },

    #[serde(rename = "signaled")]
    Signaled { signal: i32 },
}
