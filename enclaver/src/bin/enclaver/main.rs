use anyhow::Result;
use clap::{Parser, Subcommand};
use enclaver::build::EnclaveArtifactBuilder;

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
}


async fn run(args: Cli) -> Result<()> {
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

            Ok(())
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
