use tokio::task::JoinHandle;
use anyhow::Result;

use enclaver::proxy::ingress::EnclaveProxy;
use crate::config::{Configuration, ListenerConfig};

pub struct IngressService {
    proxies: Vec<JoinHandle<()>>,
}

impl IngressService {
    pub fn start(config: &Configuration) -> Result<Self> {
        let mut tasks = Vec::new();

        for (port, cfg) in &config.listener_configs {
            match cfg {
                ListenerConfig::TCP => {
                    let proxy = EnclaveProxy::bind(*port)?;
                    tasks.push(tokio::spawn(proxy.serve()));
                },
                ListenerConfig::TLS(tls_cfg) => {
                    let proxy = EnclaveProxy::bind_tls(*port, tls_cfg.clone())?;
                    tasks.push(tokio::spawn(proxy.serve()));
                },
            }
        }

        Ok(Self{
            proxies: tasks,
        })
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

