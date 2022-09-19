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
        #[clap(short, long)]
        file: String,
    },
}

async fn run(args: Cli) -> enclaver::error::Result<()> {
    match args.subcommand {
        Commands::Build { file } => {
            let builder = EnclaveArtifactBuilder::new()?;

            builder.build_artifact(&file).await
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
