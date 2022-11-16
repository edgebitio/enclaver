use std::error::Error as StdError;

use http::Uri;
use hyper::body::HttpBody;
use hyper::client::{Client, HttpConnector};
use hyper_proxy::{Intercept, Proxy, ProxyConnector};

pub type HttpProxyClient<B> = Client<ProxyConnector<HttpConnector>, B>;

/// Creates an HTTPS client that uses a proxy
pub fn new_http_proxy_client<B>(proxy_uri: Uri) -> HttpProxyClient<B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    let proxy = Proxy::new(Intercept::All, proxy_uri);
    let connector = HttpConnector::new();
    let proxy_connector = ProxyConnector::from_proxy(connector, proxy).unwrap();

    Client::builder().build(proxy_connector)
}
