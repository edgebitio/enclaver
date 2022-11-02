use anyhow::Result;
use clap::Parser;
use enclaver::run::{Enclave, EnclaveExitStatus, EnclaveOpts};
use log::info;
use std::{
    path::PathBuf,
    process::{ExitCode, Termination},
};
use tokio_util::sync::CancellationToken;

const ENCLAVE_SIGNALED_EXIT_CODE: u8 = 107;
const ENCLAVE_FATAL: u8 = 108;
const ENCLAVER_INTERRUPTED: u8 = 109;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(long, parse(from_os_str))]
    eif_file: Option<PathBuf>,

    #[clap(long, parse(from_os_str))]
    manifest_file: Option<PathBuf>,

    #[clap(long)]
    cpu_count: Option<i32>,

    #[clap(long)]
    memory_mb: Option<i32>,

    #[clap(long)]
    debug_mode: bool,
}

enum CLISuccess {
    EnclaveStatus(EnclaveExitStatus),
}

impl Termination for CLISuccess {
    fn report(self) -> ExitCode {
        match self {
            CLISuccess::EnclaveStatus(EnclaveExitStatus::Exited(code)) => {
                ExitCode::from(code as u8)
            }
            CLISuccess::EnclaveStatus(EnclaveExitStatus::Signaled(_signal)) => {
                ExitCode::from(ENCLAVE_SIGNALED_EXIT_CODE)
            }
            CLISuccess::EnclaveStatus(EnclaveExitStatus::Fatal(_err)) => {
                ExitCode::from(ENCLAVE_FATAL)
            }
            CLISuccess::EnclaveStatus(EnclaveExitStatus::Cancelled) => {
                ExitCode::from(ENCLAVER_INTERRUPTED)
            }
        }
    }
}

async fn run(args: Cli) -> Result<CLISuccess> {
    let shutdown_signal = enclaver::utils::register_shutdown_signal_handler().await?;

    let enclave = Enclave::new(EnclaveOpts {
        eif_path: args.eif_file,
        manifest_path: args.manifest_file,
        cpu_count: args.cpu_count,
        memory_mb: args.memory_mb,
        debug_mode: args.debug_mode,
    })
    .await?;

    let cancellation = CancellationToken::new();

    // Wait for the shutdown signal in a separate task. If the signal comes, cancel the
    // enclave run.
    let cancel_task = {
        let cancellation = cancellation.clone();
        tokio::task::spawn(async move {
            shutdown_signal.await;
            cancellation.cancel();
            info!("shutdown signal received, terminating enclave");
        })
    };

    let status = enclave.run(cancellation).await?;

    cancel_task.abort();
    _ = cancel_task.await;

    Ok(CLISuccess::EnclaveStatus(status))
}

#[tokio::main]
async fn main() -> Result<CLISuccess> {
    enclaver::utils::init_logging();

    let args = Cli::parse();

    run(args).await
}
