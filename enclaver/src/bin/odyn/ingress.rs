use anyhow::Result;
use ignore_result::Ignore;
use log::info;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::config::{Configuration, ListenerConfig};
use enclaver::proxy::ingress::EnclaveProxy;

pub struct IngressService {
    proxies: Vec<JoinHandle<()>>,
    shutdown: watch::Sender<()>,
}

impl IngressService {
    pub fn start(config: &Configuration) -> Result<Self> {
        let mut tasks = Vec::new();

        let (tx, rx) = tokio::sync::watch::channel(());
        for (port, cfg) in &config.listener_configs {
            match cfg {
                ListenerConfig::TCP => {
                    info!("Starting TCP ingress on port {}", *port);
                    let proxy = EnclaveProxy::bind(*port)?;
                    tasks.push(tokio::spawn(proxy.serve(rx.clone())));
                }
                ListenerConfig::TLS(tls_cfg) => {
                    info!("Starting TLS ingress on port {}", *port);
                    let proxy = EnclaveProxy::bind_tls(*port, tls_cfg.clone())?;
                    tasks.push(tokio::spawn(proxy.serve(rx.clone())));
                }
            }
        }

        Ok(Self {
            proxies: tasks,
            shutdown: tx,
        })
    }

    pub async fn stop(self) {
        self.shutdown.send(()).ignore();

        for p in self.proxies {
            p.await.ignore();
        }
    }
}
