use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::fs::File;

use tokio::io::AsyncReadExt;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    pub name: String,
    pub images: Images,
    pub ingress: Option<Vec<Ingress>>,
    pub egress: Option<Egress>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Images {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ingress {
    pub listen_port: u16,
    pub tls: Option<ServerTls>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerTls {
    pub key_file: String,
    pub cert_file: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Egress {
    pub proxy_port: Option<u16>,
    pub allow: Option<Vec<String>>,
    pub deny: Option<Vec<String>>,
}

fn parse_manifest(buf: &[u8]) -> Result<Manifest> {
    let manifest: Manifest = serde_yaml::from_slice(buf)?;

    Ok(manifest)
}

pub async fn load_manifest(path: &str) -> Result<Manifest> {
    let mut file = match File::open(path).await {
        Ok(file) => file,
        Err(err) => return Err(anyhow::anyhow!("failed to open {path}: {err}")),
    };

    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await?;

    parse_manifest(&buf).map_err(|e| anyhow!("invalid configuration in {path}: {e}"))
}

#[cfg(test)]
mod tests {
    use crate::manifest::parse_manifest;

    #[test]
    fn test_parse_manifest_with_unknown_fields() {
        assert!(parse_manifest(br#"{"foo": "bar"}"#).is_err());
    }
}