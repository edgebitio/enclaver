use anyhow::Result;
use clap::{Parser, Subcommand};
use enclaver::build::EnclaveArtifactBuilder;
use enclaver::run::Enclave;
use tokio::signal::unix::{signal, SignalKind};

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
        policy_file: String,

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
            policy_file,
            eif_file: None,
        } => {
            let builder = EnclaveArtifactBuilder::new()?;

            let (eif_info, release_img) = builder.build_release(&policy_file).await?;

            println!("Built Release Image: {}", release_img);
            println!("EIF Info: {:#?}", eif_info);

            Ok(())
        }

        Commands::Build {
            policy_file,
            eif_file: Some(eif_file),
        } => {
            let builder = EnclaveArtifactBuilder::new()?;

            let (eif_info, eif_path) = builder.build_eif_only(&policy_file, &eif_file).await?;

            println!("Built EIF: {}", eif_path.display());
            println!("EIF Info: {:#?}", eif_info);

            Ok(())
        }

        // run-eif without --debug-mode
        Commands::RunEIF {
            eif_file,
            cpu_count,
            memory_mb,
            debug_mode: false,
        } => {
            let mut enclave = Enclave::new(&eif_file, cpu_count, memory_mb);

            enclave.start().await?;

            tokio::select! {
                _ = await_shutdown_signal() => {
                    println!("Terminating Enclave...");
                    enclave.stop().await?;
                },
                _ = enclave.wait() => {
                    println!("Enclave exited");
                },
            };

            Ok(())
        }

        // run-eif with --debug-mode
        Commands::RunEIF {
            eif_file,
            cpu_count,
            memory_mb,
            debug_mode: true,
        } => {
            let enclave = Enclave::new(&eif_file, cpu_count, memory_mb);

            tokio::select! {
                _ = await_shutdown_signal() => {
                    println!("Terminating Enclave...");
                    enclave.stop().await?;
                },
                _ =  enclave.run_with_debug() => {
                    println!("Enclave exited");
                },
            };

            Ok(())
        }
    }
}

async fn await_shutdown_signal() -> Result<()> {
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    if let Err(err) = run(args).await {
        println!("error: {}", err);
        std::process::exit(1);
    }
}
