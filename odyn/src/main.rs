use log::{info, error};
use std::ffi::OsString;
use clap::{Parser};
use anyhow::Result;

pub mod enclave;
pub mod nsm;
pub mod console;
pub mod launcher;

#[derive(Parser)]
struct CliArgs {
    #[clap(long = "no-bootstrap", action)]
    no_bootstrap: bool,

    #[clap(long = "no-console", action)]
    no_console: bool,

    #[clap()]
    entrypoint: Vec<OsString>,
}

async fn run(args: &CliArgs) -> Result<()> {
    if !args.no_bootstrap {
        enclave::bootstrap().await?;
        info!("Enclave initialized");
    }

    let mut console_task = None;
    if !args.no_console {
        let (log_w, mut log_s, _log_r) = console::new_app_log()?;
        log_w.redirect_stdio()?;

        console_task = Some(tokio::spawn(async move {
            log_s.run().await
        }));
    }

    info!("Starting {:?}", args.entrypoint);
    let entrypoint = args.entrypoint.clone();
    let creds = launcher::Credentials{
        uid: 100,
        gid: 100,
    };
    tokio::task::spawn_blocking(move || {
        launcher::run_child(&entrypoint, &creds)
    }).await??;

    if let Some(task) = console_task {
        task.abort();
        task.await??;
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
