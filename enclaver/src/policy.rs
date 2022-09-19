use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::fs::File;
use tokio::io;
use tokio::io::AsyncReadExt;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unable to read policy file")]
    IO(#[from] io::Error),

    #[error("unable to parse policy file")]
    Deserialization(#[from] serde_yaml::Error),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub version: String,
    pub name: String,
    pub image: String,
}

pub async fn load_policy(path: &str) -> Result<Policy, Error> {
    let mut file = File::open(path).await?;
    let mut buf = Vec::new();

    file.read_to_end(&mut buf).await?;

    let policy: Policy = serde_yaml::from_slice(&buf)?;

    Ok(policy)
}
