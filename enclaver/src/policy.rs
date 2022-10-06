use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs::File;

use tokio::io::AsyncReadExt;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub version: String,
    pub name: String,
    pub image: String,
    pub ingress: Option<Vec<Ingress>>,
    pub egress: Option<Egress>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ingress {
    pub listen_port: u16,
    pub tls: ServerTls,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerTls {
    pub key_file: String,
    pub cert_file: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Egress {
    pub enabled: Option<bool>,
    pub proxy_port: Option<u16>,
    pub allow: Option<Vec<String>>,
}

pub async fn load_policy(path: &str) -> Result<Policy> {
    let mut file = File::open(path).await?;
    let mut buf = Vec::new();

    file.read_to_end(&mut buf).await?;

    let policy: Policy = serde_yaml::from_slice(&buf)?;

    Ok(policy)
}
