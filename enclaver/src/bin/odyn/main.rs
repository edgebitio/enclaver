pub mod config;
pub mod enclave;
pub mod nsm;
pub mod console;
pub mod launcher;
pub mod ingress;
pub mod egress;

use log::{info, error};
use std::ffi::OsString;
use clap::{Parser};
use anyhow::{Result};

use console::{AppLog, AppStatus};
use config::Configuration;
use ingress::IngressService;
use egress::EgressService;

// start "internal" ports above the 16-bit boundary (reserved for proxying TCP)
const STATUS_PORT: u32 = 17000;
const APP_LOG_PORT: u32 = 17001;

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
    let config = Configuration::load(&args.config_dir).await?;

    let mut console_task = None;
    if !args.no_console {
        let app_log = AppLog::with_stdio_redirect()?;
        console_task = Some(app_log.start_serving(APP_LOG_PORT));
    }

    let app_status = AppStatus::new();
    let app_status_task = app_status.start_serving(STATUS_PORT);

    if !args.no_bootstrap {
        enclave::bootstrap().await?;
        info!("Enclave initialized");
    }

    info!("Startng egress");
    let egress = EgressService::start(&config).await?;
    info!("Startng ingress");
    let ingress = IngressService::start(&config)?;

    let creds = launcher::Credentials{
        uid: 100,
        gid: 100,
    };

    info!("Starting {:?}", args.entrypoint);
    let exit_status = launcher::start_child(args.entrypoint.clone(), creds).await??;
    info!("Entrypoint {}", exit_status);

    info!("Stopping ingress");
    ingress.stop().await;
    info!("Stopping egress");
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
    pretty_env_logger::init();

    let args = CliArgs::parse();

    if let Err(err) = run(&args).await {
        error!("Error: {}", err);
        std::process::exit(1);
    }
}
