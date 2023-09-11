use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use enclaver::{
    build::EnclaveArtifactBuilder, constants::MANIFEST_FILE_NAME, manifest::load_manifest,
    run_container::RunWrapper,
};
use log::{debug, error};
use tokio::io::{stdout, AsyncWriteExt};

#[derive(Debug, Parser)]
#[clap(author, version)]
/// Package and run applications in Nitro Enclaves.
struct Cli {
    #[clap(subcommand)]
    subcommand: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[clap(name = "build")]
    /// Package a Docker image into a self-executing Enclaver container image.
    Build {
        #[clap(long = "file", short = 'f', default_value = "enclaver.yaml")]
        /// Path to the Enclaver manifest file to build from.
        manifest_file: String,

        #[clap(long = "eif-only", hidden = true)]
        /// Only build the EIF file, do not package it into a self-executing image.
        eif_file: Option<String>,

        #[clap(long = "pull")]
        /// Pull every container image to ensure the latest version
        force_pull: bool,
    },

    #[clap(name = "run")]
    /// Run a packaged Enclaver container image without typing long Docker commands.
    ///
    /// This command is a convenience utility that runs a pre-existing Enclaver image
    /// in the local Docker Daemon. It is equivalent to running the image with Docker,
    /// and passing:
    ///
    ///     '--device=/dev/nitro_enclaves:/dev/nitro_enclaves:rw'.
    ///
    /// Requires a local Docker Daemon to be running, and that this computer is an AWS
    /// instance configured to support Nitro Enclaves.
    Run {
        #[clap(long = "file", short = 'f')]
        /// Enclaver Manifest file in which to look for an image name.
        ///
        /// Defaults to enclaver.yaml if not set and no image is specified. To run a specific
        /// image instead, pass the name of the image as an argument.
        manifest_file: Option<String>,

        #[clap(index = 1, name = "image")]
        /// Name of a pre-existing Enclaver image to run.
        ///
        /// To automatically look this value up from an Enclaver manifest, use -f, or
        /// execute this command with an enclaver.yaml file in the current directory.
        image_name: Option<String>,

        #[clap(short = 'p', long = "publish")]
        /// Port to expose on the host machine, for example: 8080:80.
        port_forwards: Vec<String>,
    },
}

async fn run(args: Cli) -> Result<()> {
    match args.subcommand {
        // Build an OCI image based on a manifest file.
        Commands::Build {
            manifest_file,
            eif_file: None,
            force_pull,
        } => {
            let builder = EnclaveArtifactBuilder::new(force_pull)?;
            let (eif_info, release_img, tag) = builder.build_release(&manifest_file).await?;
            let eif_info_bytes = serde_json::to_vec_pretty(&eif_info)?;

            println!("Built Release Image: {release_img} ({tag})");
            println!("EIF Info:");

            stdout().write_all(&eif_info_bytes).await?;
            println!();

            Ok(())
        }

        // Build an EIF file based on a manifest file (useful for debugging, not meant for production use).
        Commands::Build {
            manifest_file,
            eif_file: Some(eif_file),
            force_pull,
        } => {
            let builder = EnclaveArtifactBuilder::new(force_pull)?;
            let (eif_info, eif_path) = builder.build_eif_only(&manifest_file, &eif_file).await?;
            let eif_info_bytes = serde_json::to_vec_pretty(&eif_info)?;

            println!("Built EIF: {}", eif_path.display());
            println!("EIF Info:");

            stdout().write_all(&eif_info_bytes).await?;
            println!();

            Ok(())
        }

        // Run an enclaver image.
        Commands::Run {
            manifest_file,
            image_name,
            port_forwards,
        } => {
            let image_name = match (manifest_file, image_name) {
                // If an image was specified, use it
                (None, Some(image_name)) => Ok(image_name),

                // If no image was specified, either use the specified manifest file or the default
                // to try to look up the target image name.
                (manifest_file, None) => {
                    let manifest_file =
                        manifest_file.unwrap_or_else(|| MANIFEST_FILE_NAME.to_string());
                    let manifest = load_manifest(manifest_file).await?;
                    Ok(manifest.target)
                }

                // Specifying both is an error
                (Some(_), Some(_)) => Err(anyhow!(
                    "both an image name and a manifest file were specified"
                )),
            }?;

            let mut runner = RunWrapper::new()?;

            let shutdown_signal = enclaver::utils::register_shutdown_signal_handler().await?;

            tokio::select! {
                res = runner.run_enclaver_image(&image_name, port_forwards) => {
                    debug!("enclave exited");
                    match res {
                        Ok(_) => debug!("enclave exited successfully"),
                        Err(e) => error!("error running enclave: {e:#}"),
                    }
                }
                _ = shutdown_signal => {
                    debug!("signal received, cleaning up...");
                }
            }

            runner.cleanup().await?;

            Ok(())
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    enclaver::utils::init_logging();

    let args = Cli::parse();

    #[cfg(feature = "tracing")]
    console_subscriber::ConsoleLayer::builder()
        .with_default_env()
        .server_addr(([127, 0, 0, 1], 51002));

    run(args).await
}
