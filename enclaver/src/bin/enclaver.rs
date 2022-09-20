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

async fn run(args: Cli) -> enclaver::error::Result<()> {
    match args.subcommand {
        Commands::Build {
            policy_file,
            eif_file: None,
        } => {
            let builder = EnclaveArtifactBuilder::new()?;

            let (eif_info, release_img) = builder.build_release(&policy_file).await?;

            println!("built release image: {}", release_img);
            println!("EIF Info: {:#?}", eif_info);

            Ok(())
        }

        Commands::Build {
            policy_file,
            eif_file: Some(eif_file),
        } => {
            let builder = EnclaveArtifactBuilder::new()?;

            let eif_info = builder.build_eif_only(&policy_file, &eif_file).await?;

            println!("built EIF: {}", eif_file);
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
