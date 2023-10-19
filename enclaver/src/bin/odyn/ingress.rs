use anyhow::Result;
use log::info;
use tokio::task::JoinHandle;

use crate::config::{Configuration, ListenerConfig};
use enclaver::proxy::ingress::EnclaveProxy;

pub struct IngressService {
    proxies: Vec<JoinHandle<()>>,
}

impl IngressService {
    pub fn start(config: &Configuration) -> Result<Self> {
        let mut tasks = Vec::new();

        for (port, cfg) in &config.listener_configs {
            match cfg {
                ListenerConfig::TCP => {
                    info!("Starting TCP ingress on port {}", *port);
                    let proxy = EnclaveProxy::bind(*port)?;
                    tasks.push(tokio::spawn(proxy.serve()));
                }
                ListenerConfig::TLS(tls_cfg) => {
                    info!("Starting TLS ingress on port {}", *port);
                    let proxy = EnclaveProxy::bind_tls(*port, tls_cfg.clone())?;
                    tasks.push(tokio::spawn(proxy.serve()));
                }
            }
        }

        Ok(Self { proxies: tasks })
    }

    pub async fn stop(self) {
        for p in &self.proxies {
            p.abort();
        }

        for p in self.proxies {
            _ = p.await;
        }
    }
}
