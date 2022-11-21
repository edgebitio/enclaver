pub mod api;
pub mod config;
pub mod console;
pub mod egress;
pub mod enclave;
pub mod ingress;
pub mod kms_proxy;
pub mod launcher;

use anyhow::Result;
use clap::Parser;
use log::{error, info};
use std::ffi::OsString;
use std::sync::Arc;

use enclaver::constants::{APP_LOG_PORT, STATUS_PORT};
use enclaver::nsm::Nsm;

use api::ApiService;
use config::Configuration;
use console::{AppLog, AppStatus};
use egress::EgressService;
use ingress::IngressService;
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

async fn launch(args: &CliArgs) -> Result<launcher::ExitStatus> {
    let config = Arc::new(Configuration::load(&args.config_dir).await?);

    let nsm = Arc::new(Nsm::new());

    if !args.no_bootstrap {
        enclave::bootstrap(nsm.clone()).await?;
        info!("Enclave initialized");
    }

    let egress = EgressService::start(&config).await?;
    let ingress = IngressService::start(&config)?;
    let kms_proxy = KmsProxyService::start(config.clone(), nsm.clone()).await?;
    let api = ApiService::start(&config, nsm.clone())?;

    let creds = launcher::Credentials { uid: 0, gid: 0 };

    info!("Starting {:?}", args.entrypoint);
    let exit_status = launcher::start_child(args.entrypoint.clone(), creds).await??;
    info!("Entrypoint {}", exit_status);

    api.stop().await;
    kms_proxy.stop().await;
    ingress.stop().await;
    egress.stop().await;

    Ok(exit_status)
}

async fn run(args: &CliArgs) -> Result<()> {
    // Start the status and logs listeners ASAP so that if we fail to
    // initialize, we can communicate the status and stream the logs
    let app_status = AppStatus::new();
    let app_status_task = app_status.start_serving(STATUS_PORT);

    let mut console_task = None;
    if !args.no_console {
        let app_log = AppLog::with_stdio_redirect()?;
        console_task = Some(app_log.start_serving(APP_LOG_PORT));
    }

    match launch(args).await {
        Ok(exit_status) => app_status.exited(exit_status),
        Err(err) => app_status.fatal(err.to_string()),
    };

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
