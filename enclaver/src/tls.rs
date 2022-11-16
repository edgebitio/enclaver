use anyhow::{anyhow, Result};
use log::info;
use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore, ServerConfig};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

fn load_certs(path: &Path) -> Result<Vec<Certificate>> {
    rustls_pemfile::certs(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("invalid cert"))
        .map(|mut certs| certs.drain(..).map(Certificate).collect())
}

fn load_keys(path: &Path) -> Result<Vec<PrivateKey>> {
    let mut key_bufs = rustls_pemfile::pkcs8_private_keys(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("invalid key"))?;

    let keys: Vec<PrivateKey> = key_bufs.drain(..).map(|buf| PrivateKey(buf)).collect();

    info!("Loaded {} TLS keys", keys.len());

    Ok(keys)
}

pub fn load_server_config(
    key: impl AsRef<Path>,
    cert: impl AsRef<Path>,
) -> Result<Arc<ServerConfig>> {
    let certs = load_certs(cert.as_ref())?;
    let mut keys = load_keys(key.as_ref())?;

    Ok(Arc::new(
        rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, keys.remove(0))?,
    ))
}

pub fn load_client_config(cert: impl AsRef<Path>) -> Result<Arc<ClientConfig>> {
    let mut roots = RootCertStore::empty();
    let certs = load_certs(cert.as_ref())?;
    roots.add(&certs[0])?;

    Ok(Arc::new(
        ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

// from rustls example code
pub struct NoCertificateVerification {}

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
}

pub fn load_insecure_client_config() -> Result<Arc<ClientConfig>> {
    let roots = RootCertStore::empty();

    let mut cfg = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots)
        .with_no_client_auth();

    cfg.dangerous()
        .set_certificate_verifier(Arc::new(NoCertificateVerification {}));

    Ok(Arc::new(cfg))
}

#[cfg(test)]
fn data_file(name: &str) -> Result<std::path::PathBuf> {
    let mut path = std::path::PathBuf::from(file!()).canonicalize()?;
    path.pop(); // pop the filename of the .rs file
    path.push(name);
    Ok(path)
}

#[cfg(test)]
pub fn test_server_config() -> Result<Arc<ServerConfig>> {
    load_server_config(data_file("test.key")?, data_file("test.crt")?)
}
