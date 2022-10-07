use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs::File;

use tokio::io::AsyncReadExt;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
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
    pub proxy_port: Option<u16>,
    pub allow: Option<Vec<String>>,
    pub deny: Option<Vec<String>>,
}

pub async fn load_manifest(path: &str) -> Result<Manifest> {
    let mut file = match File::open(path).await {
        Ok(file) => file,
        Err(err) => return Err(anyhow::anyhow!("Failed to open {path}: {err}")),
    };
    let mut buf = Vec::new();

    file.read_to_end(&mut buf).await?;

    let manifest: Manifest = serde_yaml::from_slice(&buf)?;

    Ok(manifest)
}
