use std::path::PathBuf;
use clap::{Parser, Subcommand};
use enclaver::images::{FileBuilder, FileSource, ImageManager, LayerBuilder};
use enclaver::policy::load_policy;

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

async fn run(args: Cli) -> anyhow::Result<()> {
    match args.subcommand {
        Commands::Build { file} => {
            println!("building from {file}");

            let policy = load_policy(&file).await?;
            let image_manager = ImageManager::new()?;
            let source_img = image_manager.image(&policy.image).await?;
            let res_image = image_manager.append_layer(&source_img, LayerBuilder::new()
                .append_file(FileBuilder{
                    path: PathBuf::from("/etc/enclaver/policy.yaml"),
                    source: FileSource::Local {
                        path: PathBuf::from(&file),
                    },
                    chown: "100:100".to_string(),
                })).await?;

            println!("image: {}", res_image);
        }
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
