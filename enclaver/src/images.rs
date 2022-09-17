use std::path::{PathBuf};
use bollard::Docker;
use bollard::models::ImageInspect;
use tempfile::TempDir;
use tokio::fs::{create_dir, hard_link, File};
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufWriter};
use std::fmt::Write;

#[derive(Debug)]
pub struct ImageRef {
    name: String,
    inspect: ImageInspect,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("docker daemon error")]
    Daemon(#[from] bollard::errors::Error),

    #[error("unsupported filename encoding: `{0}`")]
    FilenameEncoding(String),

    #[error("invalid format")]
    Format(#[from] std::fmt::Error),

    #[error(transparent)]
    StripPrefix(#[from] std::path::StripPrefixError),
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct ImageManager {
    docker: Docker,
}


impl ImageManager {
    pub fn new() -> Result<Self> {
        let docker_client = Docker::connect_with_local_defaults()?;

        Ok(Self{
            docker: docker_client,
        })
    }

    pub async fn image(&self, img_name: &str) -> Result<ImageRef> {
        let img = self.docker.inspect_image(img_name).await?;

        Ok(ImageRef{
            name: String::from(img_name),
            inspect: img,
        })
    }

    pub async fn append_layer(&self, img: &ImageRef, layer: &LayerBuilder) -> Result<ImageRef> {
        println!("yoo2");
        let dir = layer.realize(&img.name).await?;
        let path  = dir.into_path();
        println!("tmp dir: {}", path.to_string_lossy());

        todo!()
    }
}

pub enum FileSource {
    Local {
        path: PathBuf,
    },
    Image {
        name: String,
        path: PathBuf,
    }
}

pub struct FileBuilder {
    pub path: PathBuf,
    pub source: FileSource,
    pub chown: String,
}

impl FileBuilder {
    fn realize_to_copy_line(&self) -> Result<String> {
        let mut line = String::from("COPY");
        let dst_path = self.path.strip_prefix("/")?.to_str().ok_or({
            Error::FilenameEncoding(String::from(self.path.to_string_lossy()))
        })?;

        write!(&mut line, " --chown={}", self.chown)?;

        match &self.source {
            FileSource::Local { .. } => {
                write!(&mut line, " files/{}", dst_path)?;
            }
            FileSource::Image { name: image_name, path } => {
                let src_path = path.to_str().ok_or({
                    Error::FilenameEncoding(String::from(self.path.to_string_lossy()))
                })?;

                write!(&mut line, " --from={} {}", image_name, src_path)?;
            }
        }

        write!(&mut line, " {}\n", dst_path)?;

        Ok(line)
    }
}

pub struct LayerBuilder {
    files: Vec<FileBuilder>,
}

impl LayerBuilder {
    pub fn new() -> Self {
        Self {
            files: vec![],
        }
    }

    pub fn append_file(&mut self, file: FileBuilder) {
        self.files.push(file)
    }

    async fn realize(&self, source_image_name: &str) -> Result<TempDir> {
        // Create a temporary directory for use as a Docker context
        let tempdir = TempDir::new()?;
        let local_files = tempdir.path().join("files");
        println!("expected: {}", local_files.to_string_lossy());
        create_dir(&local_files).await?;


        let mut dw = BufWriter::new(File::create(tempdir.path().join("Dockerfile")).await?);

        dw.write(format!("FROM {source_image_name}\n\n").as_bytes()).await?;

        for file in &self.files {
            // For local files, hard link them into the `files` directory
            // in our context directory.
            if let FileSource::Local { path: source_path } = &file.source {
                let target = local_files.join(file.path.strip_prefix("/")?);
                tokio::fs::create_dir_all(target.parent().unwrap()).await?;
                println!("building hard link: {} -> {}", source_path.to_string_lossy(), target.to_string_lossy());
                hard_link(source_path, target).await?;
            }

            dw.write(file.realize_to_copy_line()?.as_bytes()).await?;
        }

        dw.flush().await?;

        Ok(tempdir)
    }
}