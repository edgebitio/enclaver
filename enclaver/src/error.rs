use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Deserialization(#[from] serde_yaml::Error),

    #[error("docker daemon error: {0}")]
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
