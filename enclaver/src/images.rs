use crate::error::{Error, Result};
use bollard::image::{BuildImageOptions, TagImageOptions};
use bollard::models::ImageId;
use bollard::Docker;
use futures_util::stream::{StreamExt, TryStreamExt};
use std::fmt;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{create_dir, hard_link, File};
use tokio::io::{duplex, AsyncWrite, AsyncWriteExt, BufWriter};
use tokio_util::codec;

#[derive(Debug)]
pub struct ImageRef {
    id: String,
}

impl ImageRef {
    pub fn to_str(&self) -> &str {
        return &self.id;
    }
}

impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// An interface for manipulating Docker images.
pub struct ImageManager {
    docker: Arc<Docker>,
}

impl ImageManager {
    /// Constructs a new ImageManager pointing to a local Docker daemon.
    pub fn new() -> Result<Self> {
        let docker_client = Arc::new(Docker::connect_with_local_defaults()?);

        Ok(Self {
            docker: docker_client,
        })
    }

    /// Constructs a new ImageManager pointing to a local Docker daemon.
    pub fn new_with_docker(docker: Arc<Docker>) -> Result<Self> {
        Ok(Self { docker })
    }

    /// Resolves a name-like string to an ImageRef referencing a specific immutable image.
    pub async fn image(&self, name: &str) -> Result<ImageRef> {
        let img = self.docker.inspect_image(name).await?;

        match img.id {
            Some(id) => Ok(ImageRef { id }),
            None => Err(Error::InvalidDaemonResponse(String::from(
                "missing image ID in image_inspect result",
            ))),
        }
    }

    /// Build and append a new layer to an image.
    ///
    /// This works by converting `layer` to a docker build operation, and executing
    /// that operation against the connected docker daemon.
    pub async fn append_layer<'a>(&self, img: &ImageRef, layer: &LayerBuilder) -> Result<ImageRef> {
        // We're going to realize `layer` to a docker context, in the form of a tarball.
        // Rather than realizing the full tarball into memory, we'll construct a pipe-like
        // pair of streams, and lazily write the tarball to one of them while streaming
        // the other end of the pipe into the daemon request.
        let (tar_write, tar_read) = duplex(1024);
        let byte_stream = codec::FramedRead::new(tar_read, codec::BytesCodec::new()).map(|r| {
            let bytes = r.unwrap().freeze();
            Ok::<_, tokio::io::Error>(bytes)
        });

        let body = hyper::Body::wrap_stream(byte_stream);

        // Concurrently build the context tarball and perform the build request.
        let (realize_res, build_res) = tokio::join!(
            layer.realize(&img.id, tar_write),
            self.docker
                .build_image(
                    BuildImageOptions {
                        dockerfile: "Dockerfile",
                        ..Default::default()
                    },
                    None,
                    Some(body)
                )
                .try_collect::<Vec<_>>(),
        );

        realize_res?;

        // The build process streams back a bunch of BuildInfos. One of them
        // should contain the ID of the resulting image; hunt through them and
        // find it.
        let build_infos = build_res?;
        let mut maybe_id = None;

        for info in &build_infos {
            if let Some(ImageId { id: Some(id) }) = &info.aux {
                maybe_id = Some(id);
                break;
            }
        }

        match maybe_id {
            Some(image_id) => self.image(image_id).await,
            None => Err(Error::InvalidDaemonResponse(String::from(
                "missing image ID",
            ))),
        }
    }

    /// Tag an image.
    pub async fn tag_image(&self, img: &ImageRef, tag: &str) -> Result<()> {
        self.docker
            .tag_image(
                img.to_str(),
                Some(TagImageOptions {
                    repo: tag,
                    ..Default::default()
                }),
            )
            .await?;

        Ok(())
    }
}

pub enum FileSource {
    Local { path: PathBuf },
    Image { name: String, path: PathBuf },
}

pub struct FileBuilder {
    pub path: PathBuf,
    pub source: FileSource,
    pub chown: String,
}

impl FileBuilder {
    fn realize_to_copy_line(&self) -> Result<String> {
        let mut line = String::from("COPY");
        let dst_path = self
            .path
            .strip_prefix("/")?
            .to_str()
            .ok_or(Error::FilenameEncoding(String::from(
                self.path.to_string_lossy(),
            )))?;

        write!(&mut line, " --chown={}", self.chown)?;

        match &self.source {
            FileSource::Local { .. } => {
                write!(&mut line, " files/{}", dst_path)?;
            }
            FileSource::Image {
                name: image_name,
                path,
            } => {
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
        Self { files: vec![] }
    }

    /// Add a file to the LayerBuilder, in the form of a FileBuilder.
    pub fn append_file(&mut self, file: FileBuilder) -> &mut Self {
        self.files.push(file);
        self
    }

    /// Realize the LayerBuilder to a tarred up Docker context containing a Dockerfile
    /// which will build the requested layer, and write the resulting context to `dst`.
    ///
    /// Note that currently this builds the context on the filesystem before generating
    /// a tarball from that file tree, but in the future it could build the context directly
    /// into the tar stream.
    async fn realize<W: AsyncWrite + Unpin + Send + 'static>(
        &self,
        source_image_name: &str,
        dst: W,
    ) -> Result<()> {
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
        dw.write(format!("FROM {source_image_name}\n\n").as_bytes())
            .await?;

        for file in &self.files {
            // For local files, hard link them into the `files` directory
            // in our context directory.
            if let FileSource::Local { path: source_path } = &file.source {
                let target = local_files.join(file.path.strip_prefix("/")?);
                let target_parent = target.parent().ok_or_else(|| {
                    Error::PathError(format!(
                        "error getting parent of {}",
                        target.to_string_lossy()
                    ))
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
