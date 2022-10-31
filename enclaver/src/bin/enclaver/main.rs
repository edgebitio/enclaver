use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use enclaver::{
    build::EnclaveArtifactBuilder, constants::MANIFEST_FILE_NAME, manifest::load_manifest,
    run_container::RunWrapper,
};
use log::{debug, error};

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

        #[clap(long = "eif-only", hidden = true)]
        eif_file: Option<String>,

        #[clap(long = "--pull")]
        force_pull: bool,
    },

    #[clap(name = "run")]
    Run {
        #[clap(long = "file", short = 'f')]
        // manifest file in which to look for an image name
        manifest_file: Option<String>,

        #[clap(index = 1, name = "image")]
        image_name: Option<String>,

        #[clap(short = 'p', long = "port")]
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

            println!("Built Release Image: {release_img} ({tag})");
            println!("EIF Info: {:#?}", eif_info);

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

            println!("Built EIF: {}", eif_path.display());
            println!("EIF Info: {:#?}", eif_info);

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
                    let manifest_file = manifest_file.unwrap_or(MANIFEST_FILE_NAME.to_string());
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

    run(args).await
}
