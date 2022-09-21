use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use rustls::{ServerConfig, Certificate, PrivateKey};
use anyhow::{Result, anyhow};
use log::info;

fn load_certs(path: &Path) -> Result<Vec<Certificate>> {
    rustls_pemfile::certs(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("invalid cert"))
        .map(|mut certs| certs.drain(..).map(Certificate).collect())
}

fn load_keys(path: &Path) -> Result<Vec<PrivateKey>> {
    let mut key_bufs =
        rustls_pemfile::pkcs8_private_keys(&mut BufReader::new(File::open(path)?))
            .map_err(|_| anyhow!("invalid key"))?;

    let keys: Vec<PrivateKey> = key_bufs
        .drain(..)
        .map(|buf| PrivateKey(buf))
        .collect();

    info!("Loaded {} TLS keys", keys.len());

    Ok(keys)
}

pub fn load_tls_config(key: &Path, cert: &Path) -> Result<Arc<ServerConfig>> {
    let certs = load_certs(cert)?;
    let mut keys = load_keys(key)?;

    Ok(Arc::new(rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, keys.remove(0))?))
}

