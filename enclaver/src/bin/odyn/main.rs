pub mod config;
pub mod enclave;
pub mod console;
pub mod launcher;
pub mod ingress;
pub mod egress;
pub mod kms_proxy;

use log::{info, error};
use std::ffi::OsString;
use std::sync::Arc;
use clap::{Parser};
use anyhow::{Result};

use enclaver::constants::{APP_LOG_PORT, STATUS_PORT};
use enclaver::nsm::Nsm;

use console::{AppLog, AppStatus};
use config::Configuration;
use ingress::IngressService;
use egress::EgressService;
use kms_proxy::KmsProxyService;

#[derive(Parser)]
struct CliArgs {
    #[clap(long = "no-bootstrap", action)]
    no_bootstrap: bool,

    #[clap(long = "no-console", action)]
    no_console: bool,

    #[clap(long = "config-dir")]
    config_dir: String,

    #[clap(required = true)]
    entrypoint: Vec<OsString>,
}

async fn run(args: &CliArgs) -> Result<()> {
    let config = Arc::new(Configuration::load(&args.config_dir).await?);

    let mut console_task = None;
    if !args.no_console {
        let app_log = AppLog::with_stdio_redirect()?;
        console_task = Some(app_log.start_serving(APP_LOG_PORT));
    }

    let nsm = Arc::new(Nsm::new());

    let app_status = AppStatus::new();
    let app_status_task = app_status.start_serving(STATUS_PORT);

    if !args.no_bootstrap {
        enclave::bootstrap(nsm.clone()).await?;
        info!("Enclave initialized");
    }

    let egress = EgressService::start(&config).await?;
    let ingress = IngressService::start(&config)?;
    let kms_proxy = KmsProxyService::start(config.clone(), nsm.clone()).await?;

    let creds = launcher::Credentials{
        uid: 0,
        gid: 0,
    };

    info!("Starting {:?}", args.entrypoint);
    let exit_status = launcher::start_child(args.entrypoint.clone(), creds).await??;
    info!("Entrypoint {}", exit_status);

    kms_proxy.stop().await;
    ingress.stop().await;
    egress.stop().await;

    app_status.exited(exit_status);

    app_status_task.await??;

    if let Some(task) = console_task {
        task.abort();
        _ = task.await;
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    enclaver::utils::init_logging();
    let args = CliArgs::parse();

    if let Err(err) = run(&args).await {
        error!("Error: {err:#}");
        std::process::exit(1);
    }
}
