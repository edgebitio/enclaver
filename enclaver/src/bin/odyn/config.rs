use anyhow::Result;
use enclaver::constants::MANIFEST_FILE_NAME;
use log::debug;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use enclaver::manifest::{self, Manifest};
use enclaver::tls;

pub struct Configuration {
    pub config_dir: PathBuf,
    pub manifest: Manifest,
    pub listener_configs: HashMap<u16, ListenerConfig>,
}

#[derive(Clone)]
pub enum ListenerConfig {
    TCP,
    TLS(Arc<rustls::ServerConfig>),
}

impl Configuration {
    pub async fn load<P: AsRef<Path>>(config_dir: P) -> Result<Self> {
        let mut manifest_path = config_dir.as_ref().to_path_buf();
        manifest_path.push(MANIFEST_FILE_NAME);

        let manifest = enclaver::manifest::load_manifest(manifest_path.to_str().unwrap()).await?;

        let mut tls_path = config_dir.as_ref().to_path_buf();
        tls_path.extend(["tls", "server"]);

        let mut listener_configs = HashMap::new();

        if let Some(ref ingress) = manifest.ingress {
            for item in ingress {
                let cfg = match item.tls {
                    Some(_) => {
                        let tls_config = Configuration::load_tls_server_config(&tls_path, item)?;
                        ListenerConfig::TLS(tls_config)
                    },
                    None => ListenerConfig::TCP,
                };

                listener_configs.insert(item.listen_port, cfg);
            }
        }

        Ok(Self {
            config_dir: config_dir.as_ref().to_path_buf(),
            manifest: manifest,
            listener_configs,
        })
    }

    fn load_tls_server_config(
        tls_path: &Path,
        ingress: &manifest::Ingress,
    ) -> Result<Arc<rustls::ServerConfig>> {
        let mut ingress_path = tls_path.to_path_buf();
        ingress_path.push(&ingress.listen_port.to_string());

        let mut key_path = ingress_path.clone();
        key_path.push("key.pem");

        let mut cert_path = ingress_path.clone();
        cert_path.push("cert.pem");

        debug!("Loading key_file: {}", key_path.to_string_lossy());
        debug!("Loading cert_file: {}", cert_path.to_string_lossy());
        tls::load_server_config(key_path, cert_path)
    }
}
