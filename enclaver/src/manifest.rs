use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::fs::File;

use tokio::io::AsyncReadExt;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    pub name: String,
    pub target: String,
    pub sources: Sources,
    pub ingress: Option<Vec<Ingress>>,
    pub egress: Option<Egress>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    pub app: String,
    pub supervisor: Option<String>,
    pub wrapper: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ingress {
    pub listen_port: u16,
    pub tls: ServerTls,
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
        assert!(parse_manifest(br#"foo: "bar""#).is_err());
    }

    #[test]
    fn test_parse_minimal_manifest() {
        let raw_manifest = br#"
version: v1
name: "test"
target: "target-image:latest"
sources:
  app: "app-image:latest"
#r"#;

        let manifest = parse_manifest(raw_manifest).unwrap();

        assert_eq!(manifest.version, "v1");
        assert_eq!(manifest.name, "test");
        assert_eq!(manifest.target, "target-image:latest");
        assert_eq!(manifest.sources.app, "app-image:latest");
    }
}