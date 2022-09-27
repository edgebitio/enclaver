use tokio::task::JoinHandle;
use anyhow::Result;

use enclaver::proxy::ingress::EnclaveProxy;
use crate::config::Configuration;

pub struct IngressService {
    proxies: Vec<JoinHandle<()>>,
}

impl IngressService {
    pub fn start(config: &Configuration) -> Result<Self> {
        let mut proxies = Vec::new();

        for (port, cfg) in &config.tls_server_configs {
            proxies.push(EnclaveProxy::bind(*port, (*cfg).clone())?);
        }

        let mut tasks = Vec::new();
        for proxy in proxies {
            tasks.push(tokio::task::spawn(proxy.serve()));
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

