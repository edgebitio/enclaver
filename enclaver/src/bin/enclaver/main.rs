use anyhow::Result;
use clap::{Parser, Subcommand};
use enclaver::{build::EnclaveArtifactBuilder, run::EnclaveExitStatus};
#[cfg(feature = "run_enclave")]
use enclaver::run::{Enclave, EnclaveOpts};
use log::{debug, error, info};
use std::{future::Future, path::PathBuf, process::{Termination, ExitCode}};
use tokio::signal::unix::{signal, SignalKind};

const ENCLAVE_SIGNALED_EXIT_CODE: u8 = 107;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    subcommand: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[clap(name = "build")]
    Build {
        #[clap(long = "file", short = 'f', default_value = "enclaver.yaml")]
        manifest_file: String,

        #[clap(long = "eif-only")]
        eif_file: Option<String>,

        #[clap(long = "--pull")]
        force_pull: bool,
    },

    #[clap(name = "run-eif")]
    RunEIF {
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
    },
}

enum CLISuccess {
    Ok,
    EnclaveStatus(enclaver::run::EnclaveExitStatus),
}

impl Termination for CLISuccess {
    fn report(self) -> ExitCode {
        match self {
            CLISuccess::Ok => ExitCode::SUCCESS,
            CLISuccess::EnclaveStatus(EnclaveExitStatus::Exited(code)) => ExitCode::from(code as u8),
            CLISuccess::EnclaveStatus(EnclaveExitStatus::Signaled(_signal)) => ExitCode::from(ENCLAVE_SIGNALED_EXIT_CODE),
        }
    }
}

async fn run(args: Cli) -> Result<CLISuccess> {
    match args.subcommand {
        Commands::Build {
            manifest_file,
            eif_file: None,
            force_pull,
        } => {
            let builder = EnclaveArtifactBuilder::new(force_pull)?;
            let (eif_info, release_img, tag) = builder.build_release(&manifest_file).await?;

            println!("Built Release Image: {release_img} ({tag})");
            println!("EIF Info: {:#?}", eif_info);

            Ok(CLISuccess::Ok)
        }

        Commands::Build {
            manifest_file,
            eif_file: Some(eif_file),
            force_pull,
        } => {
            let builder = EnclaveArtifactBuilder::new(force_pull)?;

            let (eif_info, eif_path) = builder.build_eif_only(&manifest_file, &eif_file).await?;

            println!("Built EIF: {}", eif_path.display());
            println!("EIF Info: {:#?}", eif_info);

            Ok(CLISuccess::Ok)
        }

        #[cfg(feature = "run_enclave")]
        Commands::RunEIF {
            eif_file,
            manifest_file,
            cpu_count,
            memory_mb,
            debug_mode,
        } => {
            let shutdown_signal = register_shutdown_signal_handler().await?;

            let mut enclave = Enclave::new(EnclaveOpts {
                eif_path: eif_file,
                manifest_path: manifest_file,
                cpu_count,
                memory_mb,
                debug_mode,
            })
            .await?;

            tokio::select! {
                _ = shutdown_signal => {
                    info!("shutdown signal received, terminating enclave");
                },
                enclave_res = enclave.run() => {
                    match enclave_res {
                        Ok(()) => debug!("enclave exited successfully"),
                        Err(e) => error!("error running enclave: {e:#}"),
                    }
                },
            }

            if let Some(status) = enclave.cleanup().await? {
                Ok(CLISuccess::EnclaveStatus(status))
            } else {
                Ok(CLISuccess::Ok)
            }
        }

        // run-eif on unsupported platform
        #[cfg(not(feature = "run_enclave"))]
        Commands::RunEIF { .. } => {
            use anyhow::anyhow;

            Err(anyhow!(
                "Running enclaves is not supported on this platform"
            ))
        }
    }
}

async fn register_shutdown_signal_handler() -> Result<impl Future> {
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    let f = tokio::task::spawn(async move {
        tokio::select! {
            _ = sigint.recv() => (),
            _ = sigterm.recv() => (),
        }
    });

    Ok(f)
}

#[tokio::main]
async fn main() -> Result<CLISuccess> {
    enclaver::utils::init_logging();

    let args = Cli::parse();

    run(args).await
}
