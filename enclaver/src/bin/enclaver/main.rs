use std::future::Future;
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use futures_util::sink::SinkExt;
use futures_util::TryFutureExt;
use tokio::io::stderr;
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::codec::{BytesCodec, FramedWrite};

use enclaver::build::EnclaveArtifactBuilder;
#[cfg(feature = "run_enclave")]
use enclaver::run::Enclave;

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
        #[clap(long = "file", short = 'f')]
        manifest_file: String,

        #[clap(long = "eif-only")]
        eif_file: Option<String>,
    },

    #[clap(name = "run-eif")]
    RunEIF {
        #[clap(long)]
        eif_file: String,

        #[clap(long)]
        cpu_count: i32,

        #[clap(long)]
        memory_mb: i32,

        #[clap(long)]
        debug_mode: bool,
    },
}

async fn run(args: Cli) -> Result<()> {
    match args.subcommand {
        Commands::Build {
            manifest_file,
            eif_file: None,
        } => {
            let builder = EnclaveArtifactBuilder::new()?;
            let (eif_info, release_img, tag) = builder.build_release(&manifest_file).await?;

            println!("Built Release Image: {release_img} ({tag})");
            println!("EIF Info: {:#?}", eif_info);

            Ok(())
        }

        Commands::Build {
            manifest_file,
            eif_file: Some(eif_file),
        } => {
            let builder = EnclaveArtifactBuilder::new()?;

            let (eif_info, eif_path) = builder.build_eif_only(&manifest_file, &eif_file).await?;

            println!("Built EIF: {}", eif_path.display());
            println!("EIF Info: {:#?}", eif_info);

            Ok(())
        }

        // run-eif without --debug-mode
        #[cfg(feature = "run_enclave")]
        Commands::RunEIF {
            eif_file,
            cpu_count,
            memory_mb,
            debug_mode: false,
        } => {
            let mut stderr = FramedWrite::new(stderr(), BytesCodec::new());
            let mut enclave = Enclave::new(&eif_file, cpu_count, memory_mb);
            let shutdown_signal = register_shutdown_signal_handler().await?;

            enclave.start().await?;

            // Asynchronously wait for the enclave to come up, then then stream its logs to stderr
            // until the log stream ends. Don't poll this Future now, instead we do so down below
            // in the select, in order to handle any signals as soon as possible.
            // TODO(russell_h): Use the enclave status socket in parallel to more conclusively
            // determine when the enclave has exited.
            let stream_logs = enclave.wait_logs()
                .and_then(|mut log_stream| async move {
                    match stderr.send_all(&mut log_stream).await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(anyhow!(e)),
                    }
                });

            tokio::select! {
                _ = shutdown_signal => {
                    println!("Terminating Enclave...");
                    enclave.stop().await?;
                },
                _ = stream_logs => {
                    println!("Enclave exited");
                },
            }

            Ok(())
        }

        // run-eif with --debug-mode
        #[cfg(feature = "run_enclave")]
        Commands::RunEIF {
            eif_file,
            cpu_count,
            memory_mb,
            debug_mode: true,
        } => {
            let enclave = Enclave::new(&eif_file, cpu_count, memory_mb);
            let shutdown_signal = register_shutdown_signal_handler().await?;

            tokio::select! {
                _ = shutdown_signal => {
                    println!("Terminating Enclave...");
                    enclave.stop().await?;
                },
                _ =  enclave.run_with_debug() => {
                    println!("Enclave exited");
                },
            }

            Ok(())
        }

        // run-eif on unsupported platform
        #[cfg(not(feature = "run_enclave"))]
        Commands::RunEIF { .. } => Err(anyhow!(
            "Running enclaves is not supported on this platform"
        )),
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
async fn main() {
    pretty_env_logger::init();
    let args = Cli::parse();

    if let Err(err) = run(args).await {
        println!("error: {}", err);
        std::process::exit(1);
    }
}
