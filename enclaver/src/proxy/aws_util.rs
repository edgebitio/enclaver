use anyhow::{anyhow, Result};

use http::Uri;
use hyper::client::HttpConnector;
use hyper_proxy::{Intercept, Proxy, ProxyConnector};

use aws_config::imds;
use aws_config::imds::credentials::ImdsCredentialsProvider;
use aws_config::imds::region::ImdsRegionProvider;
use aws_config::provider_config::ProviderConfig;
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_smithy_client::{bounds::SmithyConnector, erase::DynConnector, hyper_ext};
use aws_smithy_http::result::ConnectorError;
use aws_types::sdk_config::SdkConfig;

const IMDS_URL: &str = "http://169.254.169.254:80/";

fn new_proxy_connector(
    proxy_uri: Uri,
) -> Result<impl SmithyConnector<Error = ConnectorError> + Send> {
    let mut proxy = Proxy::new(Intercept::All, proxy_uri);
    proxy.force_connect();

    let connector = HttpConnector::new();
    let proxy_connector = ProxyConnector::from_proxy(connector, proxy)?;
    Ok(hyper_ext::Adapter::builder().build(proxy_connector))
}

pub async fn imds_client_with_proxy(proxy_uri: Uri) -> Result<imds::Client> {
    let connector = new_proxy_connector(proxy_uri)?;

    let config = ProviderConfig::without_region().with_http_connector(DynConnector::new(connector));

    let client = imds::Client::builder()
        .configure(&config)
        .endpoint(http::Uri::from_static(IMDS_URL))
        .build()
        .await?;

    Ok(client)
}

pub async fn load_config_from_imds(imds_client: imds::Client) -> Result<SdkConfig> {
    let region = ImdsRegionProvider::builder()
        .imds_client(imds_client.clone())
        .build()
        .region()
        .await
        .ok_or(anyhow!("failed to fetch the region from IMDS"))?;

    let cred_provider = ImdsCredentialsProvider::builder()
        .imds_client(imds_client)
        .build();

    let config = SdkConfig::builder()
        .region(Some(region))
        .credentials_provider(SharedCredentialsProvider::new(cred_provider))
        .build();

    Ok(config)
}
