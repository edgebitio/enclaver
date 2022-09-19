use std::path::{PathBuf};
use std::fmt::Write;
use bollard::Docker;
use bollard::models::{ImageId};
use bollard::image::BuildImageOptions;
use tokio::fs::{create_dir, hard_link, File};
use tokio::io::{AsyncWriteExt, BufWriter, AsyncWrite, duplex};
use tokio_util::codec;
use thiserror::Error;
use futures_util::stream::{StreamExt, TryStreamExt};

#[derive(Debug)]
pub struct ImageRef {
    id: String,
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

    #[error("path error: {0}")]
    PathError(String),

    #[error("invalid response from docker: {0}")]
    InvalidDaemonResponse(String),
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

        match img.id {
            Some(id) => Ok(ImageRef{ id }),
            None => Err(Error::InvalidDaemonResponse(String::from("missing image ID in image_inspect result"))),
        }
    }

    pub async fn append_layer<'a>(&self, img: &ImageRef, layer: &LayerBuilder) -> Result<ImageRef> {
        let (tar_write, tar_read) = duplex(1024);
        let byte_stream = codec::FramedRead::new(tar_read, codec::BytesCodec::new()).map(|r| {
            let bytes = r.unwrap().freeze();
            Ok::<_, tokio::io::Error>(bytes)
        });

        let body = hyper::Body::wrap_stream(byte_stream);

        let realize_future = layer.realize(&img.id, tar_write);

        let build_future = self.docker.build_image(BuildImageOptions {
            dockerfile: "Dockerfile",
            ..Default::default()
        }, None, Some(body)).try_collect::<Vec<_>>();

        let (realize_res, build_res) = tokio::join!(realize_future, build_future);

        realize_res?;
        let build_infos = build_res?;
        let mut maybe_id = None;

        for info in &build_infos {
            if let Some(ImageId { id: Some(id) }) = &info.aux {
                maybe_id = Some(id);
                break;
            }
        };

        match maybe_id {
            Some(image_id) => self.image(image_id).await,
            None => Err(Error::InvalidDaemonResponse(String::from("missing image ID")))
        }
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

    async fn realize<W: AsyncWrite + Unpin + Send + 'static>(&self, source_image_name: &str, dst: W) -> Result<()> {
        // Create a temporary directory in which to construct a Docker context.
        let tempdir = tempfile::TempDir::new()?;

        // Create a "files" subdirectory. Within "files" we will hardlink any
        // local files to be copied to the image.
        let local_files = tempdir.path().join("files");
        create_dir(&local_files).await?;

        // We'll also write out a Dockerfile with a COPY line for each file:
        // - for local files we'll COPY from the "files" directory
        // - for image-sourced files we'll write COPY to pull from the image
        let mut dw = BufWriter::new(File::create(tempdir.path().join("Dockerfile")).await?);
        dw.write(format!("FROM {source_image_name}\n\n").as_bytes()).await?;

        for file in &self.files {
            // For local files, hard link them into the `files` directory
            // in our context directory.
            if let FileSource::Local { path: source_path } = &file.source {
                let target = local_files.join(file.path.strip_prefix("/")?);
                let target_parent = target.parent().ok_or_else(|| {
                    Error::PathError(format!("error getting parent of {}", target.to_string_lossy()))
                })?;
                tokio::fs::create_dir_all(target_parent).await?;
                hard_link(source_path, target).await?;
            }

            dw.write(file.realize_to_copy_line()?.as_bytes()).await?;
        }

        dw.flush().await?;

        // Write the entire context directory to a tarball.
        let mut tb = tokio_tar::Builder::new(dst);
        tb.append_dir_all(".", tempdir).await?;

        Ok(())
    }
}