use tokio::task::JoinHandle;
use anyhow::Result;

use enclaver::proxy::egress_http::EnclaveHttpProxy;
use crate::config::Configuration;

const HTTP_EGRESS_PROXY_PORT: u16 = 9000;
const HTTP_EGRESS_VSOCK_PORT: u32 = 17002;

pub struct EgressService {
    proxy: Option<JoinHandle<()>>,
}

impl EgressService {
    pub async fn start(config: &Configuration) -> Result<Self> {
        let proxy = match proxy_port(config) {
            Some(port) => {
                set_proxy_env_var(&format!("http://127.0.0.1:{port}"));

                let proxy = EnclaveHttpProxy::bind(HTTP_EGRESS_PROXY_PORT).await?;

                Some(tokio::task::spawn(async move {
                    proxy.serve(HTTP_EGRESS_VSOCK_PORT).await;
                }))
            },
            None => None,
        };

        Ok(Self{
            proxy: proxy,
        })
    }

    pub async fn stop(self) {
        if let Some(proxy) = self.proxy {
            proxy.abort();
            _ = proxy.await;
        }
    }
}

fn proxy_port(config: &Configuration) -> Option<u16> {
    if let Some(ref egress) = config.policy.egress {
        if egress.enabled? {
            let port = egress.proxy_port.unwrap_or(HTTP_EGRESS_PROXY_PORT);
            return Some(port)
        }
    }

    None
}

fn set_proxy_env_var(value: &str) {
    std::env::set_var("http_proxy", value);
    std::env::set_var("https_proxy", value);
    std::env::set_var("HTTP_PROXY", value);
    std::env::set_var("HTTPS_PROXY", value);
}
