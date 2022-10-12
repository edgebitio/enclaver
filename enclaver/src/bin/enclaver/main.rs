use std::future::Future;
use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use log::info;
use tokio::signal::unix::{signal, SignalKind};
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
        manifest_file: String,

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

        #[cfg(feature = "run_enclave")]
        Commands::RunEIF {
            eif_file,
            manifest_file,
            cpu_count,
            memory_mb,
            debug_mode,
        } => {
            let mut enclave = Enclave::new(&eif_file, manifest_file, cpu_count, memory_mb, debug_mode).await?;
            let shutdown_signal = register_shutdown_signal_handler().await?;

            enclave.start().await?;

            shutdown_signal.await;

            info!("shutdown signal received, terminating enclave");
            enclave.stop().await?;

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
    // This is kind of a hack...
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();
    let args = Cli::parse();

    if let Err(err) = run(args).await {
        println!("error: {err:#}");
        std::process::exit(1);
    }
}
