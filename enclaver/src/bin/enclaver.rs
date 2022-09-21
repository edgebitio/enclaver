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
        #[clap(long = "file", short = 'f')]
        policy_file: String,

        #[clap(long = "eif-only")]
        eif_file: Option<String>,
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
    }
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    if let Err(err) = run(args).await {
        println!("error: {}", err);
        std::process::exit(1);
    }
}
