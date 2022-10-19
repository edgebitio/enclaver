use std::sync::Arc;

use tokio::task::JoinHandle;
use anyhow::{Result, anyhow};
use log::{info, error};

use enclaver::nsm::Nsm;
use enclaver::keypair::KeyPair;
use enclaver::proxy::kms::{NsmAttestationProvider, KmsProxyConfig, KmsProxy};

use crate::config::Configuration;

const NO_EGRESS_ERROR: &str = "KMS proxy is configured but egress is not. Configure egress allow policy to access the IMDS at 169.254.169.254 and the AWS KMS endpoint";

pub struct KmsProxyService {
    proxy: Option<JoinHandle<()>>,
}

impl KmsProxyService {
    pub async fn start(config: &Configuration, nsm: Arc<Nsm>) -> Result<Self> {
        let task = if let Some(port) = config.kms_proxy_port() {
            if let Some(proxy_uri) = config.egress_proxy_uri() {
                let attester = Box::new(NsmAttestationProvider::new(nsm));

                // If a keypair will be needed elsewhere, this should be moved out
                info!("Generating public/private keypair");
                let keypair = Arc::new(KeyPair::generate()?);

                info!("Fetching credentials from IMDSv2");
                let kms_config = KmsProxyConfig::from_imds(proxy_uri, keypair, attester).await?;
                info!("Credentials fetched");

                let proxy = KmsProxy::bind(port, kms_config)?;

                Some(tokio::task::spawn(async move {
                    if let Err(err) = proxy.serve().await {
                        error!("Error serving KMS proxy: {err}");
                    }
                }))
            } else {
                return Err(anyhow!(NO_EGRESS_ERROR));
            }
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
