use std::sync::Arc;

use tokio::task::JoinHandle;
use anyhow::Result;

use enclaver::proxy::egress_http::EnclaveHttpProxy;
use enclaver::policy::EgressPolicy;
use crate::config::Configuration;

const HTTP_EGRESS_PROXY_PORT: u16 = 9000;
const HTTP_EGRESS_VSOCK_PORT: u32 = 17002;

pub struct EgressService {
    proxy: Option<JoinHandle<()>>,
}

impl EgressService {
    pub async fn start(config: &Configuration) -> Result<Self> {
        let task = if is_enabled(config) {
            let proxy_port = proxy_port(config);
            let policy = Arc::new(EgressPolicy::new(config.manifest.egress.as_ref().unwrap()));

            set_proxy_env_var(&format!("http://127.0.0.1:{proxy_port}"));

            let proxy = EnclaveHttpProxy::bind(proxy_port).await?;

            Some(tokio::task::spawn(async move {
                proxy.serve(HTTP_EGRESS_VSOCK_PORT, policy).await;
            }))
        } else {
            None
        };

        Ok(Self{
            proxy: task,
        })
    }

    pub async fn stop(self) {
        if let Some(proxy) = self.proxy {
            proxy.abort();
            _ = proxy.await;
        }
    }
}

fn is_enabled(config: &Configuration) -> bool {
    if let Some(ref egress) = config.manifest.egress {
        if let Some(ref allow) = egress.allow {
            return !allow.is_empty();
        }
    }

    false
}

fn proxy_port(config: &Configuration) -> u16 {
    config.manifest
        .egress.as_ref().unwrap()
        .proxy_port.unwrap_or(HTTP_EGRESS_PROXY_PORT)
}

fn set_proxy_env_var(value: &str) {
    std::env::set_var("http_proxy", value);
    std::env::set_var("https_proxy", value);
    std::env::set_var("HTTP_PROXY", value);
    std::env::set_var("HTTPS_PROXY", value);
}
