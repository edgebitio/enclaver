use std::net::{SocketAddrV4, Ipv4Addr};
use std::sync::Arc;

use log::{error, debug};
use tokio::io::{AsyncWrite, AsyncRead, AsyncWriteExt, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_vsock::VsockStream;
use futures::{Stream, StreamExt};
use hyper::client::conn::Builder;
use hyper::server::conn::Http;
use hyper::service::service_fn;
use hyper::{Method, Body, Request, Response};
use hyper::header::HeaderValue;
use http::uri::PathAndQuery;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use anyhow::anyhow;
use async_trait::async_trait;

use crate::policy::EgressPolicy;

#[async_trait]
trait JsonTransport :Sized + Sync {
    async fn send<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()>;
    async fn recv<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self>;
}

#[async_trait]
impl <M: Serialize + DeserializeOwned + Sync> JsonTransport for M {
    async fn send<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
        // Frame and serialize
        // use JSON serialization to avoid pulling in another dependency
        let msg = serde_json::to_vec(self)?;
        // frame it by a 2 byte length
        let len = msg.len() as u16;
        let mut pkt = Vec::with_capacity(2 + msg.len());
        pkt.extend_from_slice(&len.to_le_bytes());
        pkt.extend_from_slice(&msg);
        w.write_all(&pkt).await?;
        Ok(())
    }

    async fn recv<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
        let mut len_buf = [0u8; 2];
        r.read_exact(&mut len_buf).await?;
        let len = u16::from_le_bytes(len_buf);

        let mut msg = vec![0u8; len as usize];
        r.read_exact(&mut msg).await?;

        let req: Self = serde_json::from_slice(&msg)?;
        Ok(req)
    }
}

#[derive(Serialize, Deserialize)]
struct ConnectRequest {
    host: String,
    port: u16,
}

impl ConnectRequest {
    fn new(host: String, port: u16) -> Self {
        Self{
            host: host,
            port: port,
        }
    }
}

#[derive(Serialize, Deserialize)]
enum ConnectResponse {
    Ok,
    Err{
        os_code: i32,
        message: String,
    },
}

impl ConnectResponse {
    fn failed(err: &std::io::Error) -> Self {
        Self::Err{
            os_code: err.raw_os_error().unwrap_or(0i32),
            message: err.to_string(),
        }
    }
}

pub struct EnclaveHttpProxy {
    listener: TcpListener,
}

impl EnclaveHttpProxy {
    pub async fn bind(port: u16) -> anyhow::Result<Self> {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        Ok(Self{
            listener: TcpListener::bind(addr).await?,
        })
    }

    pub async fn serve(self, egress_port: u32, egress_policy: Arc<EgressPolicy>) {
        loop {
            match self.listener.accept().await {
                Ok((sock, _)) => {
                    let egress_policy = egress_policy.clone();

                    tokio::task::spawn(async move {
                        EnclaveHttpProxy::service_conn(sock, egress_port, egress_policy).await;
                    });
                },
                Err(err) => { error!("Accept failed: {err}"); },
            }
        }
    }

    async fn service_conn(tcp: TcpStream, egress_port: u32, egress_policy: Arc<EgressPolicy>) {
        let svc = service_fn(move |req| {
            let egress_policy = egress_policy.clone();
            async move {
                proxy(egress_port, req, &egress_policy).await
            }
        });

        if let Err(err) = Http::new()
            .http1_preserve_header_case(true)
            .http1_title_case_headers(true)
            .serve_connection(tcp, svc)
            .with_upgrades()
            .await
        {
            error!("Failed to serve connection: {err}");
        }
    }
}

pub struct HostHttpProxy {
    incoming: Box<dyn Stream<Item=VsockStream> + Unpin + Send>,
}

impl HostHttpProxy {
    pub fn bind(egress_port: u32) -> anyhow::Result<Self> {
        Ok(Self{
            incoming: Box::new(crate::vsock::serve(egress_port)?),
        })
    }

    pub async fn serve(self) {
        let mut incoming = Box::into_pin(self.incoming);

        while let Some(stream) = incoming.next().await {
            tokio::task::spawn(async move {
                if let Err(err) = HostHttpProxy::service_conn(stream).await {
                    error!("{err}");
                }
            });
        }
    }

    async fn service_conn(mut vsock: VsockStream) -> anyhow::Result<()> {
        let conn_req = ConnectRequest::recv(&mut vsock).await?;
        match TcpStream::connect((conn_req.host.as_ref(), conn_req.port)).await {
            Ok(mut tcp) => {
                ConnectResponse::Ok.send(&mut vsock).await?;

                debug!("Connected to {}:{}, starting to proxy bytes", conn_req.host, conn_req.port);
                _ = tokio::io::copy_bidirectional(&mut vsock, &mut tcp).await;
                debug!("Proxying is done");
            },
            Err(err) => {
                ConnectResponse::failed(&err)
                    .send(&mut vsock)
                    .await?;
                }
        }

        Ok(())
    }
}

async fn proxy(egress_port: u32, req: Request<Body>,
               egress_policy: &EgressPolicy) -> Result<Response<Body>, hyper::Error> {
    if Method::CONNECT == req.method() {
        Ok(handle_connect(egress_port, req, egress_policy).await)
    } else {
        match handle_request(egress_port, req, egress_policy).await {
            Ok(resp) => Ok(resp),
            Err(err) => Ok(err_resp(http::StatusCode::SERVICE_UNAVAILABLE, err.to_string())),
        }
    }
}

async fn handle_connect(egress_port: u32, req: Request<Body>, egress_policy: &EgressPolicy) -> Response<Body> {
    debug!("Handling CONNECT request");

    match req.uri().authority() {
        Some(authority) => {
            let port = match authority.port() {
                Some(port) => port.as_u16(),
                None => {
                    let err_msg = "CONNECT address is missing a port";
                    error!("{err_msg}");
                    return bad_request(err_msg.to_string());
                }
            };

            // Check the policy
            if !egress_policy.is_host_allowed(authority.host()) {
                return blocked();

            }

            // Connect to remote server before the upgrade so we can return an error if it fails
            let mut remote = match remote_connect(egress_port, authority.host(), port).await {
                Ok(remote) => remote,
                Err(err) => return err_resp(http::StatusCode::SERVICE_UNAVAILABLE, err.to_string()),
            };
            debug!("Connected to origin server");

            tokio::task::spawn(async move {
                match hyper::upgrade::on(req).await {
                    Ok(mut upgraded) => {
                        debug!("Connection upgraded");
                        _ = tokio::io::copy_bidirectional(&mut upgraded, &mut remote).await;
                        debug!("Proxying is done");
                    }
                    Err(err) => {
                        error!("Upgrade failed: {err}");
                    }
                }
            });

            Response::new(Body::empty())
        },
        None => {
            let err_msg = format!("CONNECT host is not socket addr: {:?}", req.uri());
            error!("{err_msg}");
            bad_request(err_msg)
        }
    }
}

async fn handle_request(egress_port: u32, mut req: Request<Body>, egress_policy: &EgressPolicy) -> anyhow::Result<Response<Body>> {
    let host = match req.uri().host() {
        Some(host) => host,
        None => return Ok(bad_request("URI is missing a host".to_string())),
    };
    let port = req.uri().port_u16().unwrap_or(80);

    // Check the policy
    if !egress_policy.is_host_allowed(host) {
        return Ok(blocked());
    }

    // TODO: pool connections
    let stream = remote_connect(egress_port, host, port).await?;

    // Set the Host: header to match the URL
    let host_hdr = match req.uri().port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    req.headers_mut().insert(hyper::header::HOST, HeaderValue::from_str(&host_hdr)?);

    // If a proxy receives an OPTIONS request with an absolute-form of
    // request-target in which the URI has an empty path and no query
    // component, then the last proxy on the request chain MUST send a
    // request-target of "*" when it forwards the request to the indicated
    // origin server.
    let pq = if req.method() == Method::OPTIONS && is_empty(req.uri().path_and_query()) {
        PathAndQuery::from_static("*")
    } else {
        // Convert the absolute-form into origin-form
        match req.uri().path_and_query() {
            Some(pq) => pq.clone(),
            None => PathAndQuery::from_static("/"),
        }
    };

    *req.uri_mut() = http::Uri::builder()
        .path_and_query(pq)
        .build()?;

    let (mut sender, conn) = Builder::new()
        .http1_preserve_header_case(true)
        .http1_title_case_headers(true)
        .handshake(stream)
        .await?;

    // Spawning detached here is not ideal but the right thing to do
    // according to the docs
    tokio::task::spawn(async move { _ = conn.await; });

    Ok(sender.send_request(req).await?)
}

fn err_resp(status: http::StatusCode, msg: String) -> Response<Body> {
    let mut resp = Response::new(Body::from(msg));
    *resp.status_mut() = status;
    resp
}

fn bad_request(msg: String) -> Response<Body> {
    err_resp(http::StatusCode::BAD_REQUEST, msg)
}

fn blocked() -> Response<Body> {
    err_resp(http::StatusCode::UNAUTHORIZED, "blocked by egress security policy".to_string())
}

fn is_empty(pq: Option<&PathAndQuery>) -> bool {
    if let Some(pq) = pq {
        if pq.path() != "/" {
            return false;
        }

        match pq.query() {
            Some(q) => q.is_empty(),
            None => true,
        }
    } else {
        true
    }
}

// connects to the host via vsock and then asks it to
// connect to the remote address
async fn remote_connect(egress_port: u32, host: &str, port: u16) -> anyhow::Result<VsockStream> {
    let mut vsock = VsockStream::connect(crate::vsock::VMADDR_CID_HOST, egress_port).await?;
    debug!("Connected to vsock {}:{}, sending connect request", crate::vsock::VMADDR_CID_HOST, egress_port);

    ConnectRequest::new(host.to_string(), port).send(&mut vsock).await?;
    debug!("Sent request to connect to {host}:{port}");

    match ConnectResponse::recv(&mut vsock).await? {
        ConnectResponse::Ok => Ok(vsock),
        ConnectResponse::Err{os_code, message} => Err(anyhow!("os_err: {os_code}: {message}")),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};
    use std::convert::Infallible;
    use std::sync::Arc;
    use std::future::Future;
    use hyper::{Request, Response, Body, Server};
    use hyper::server::conn::AddrIncoming;
    use http::{Method, Version, uri::PathAndQuery};
    use tls_listener::TlsListener;
    use tokio::task::JoinHandle;
    use rand::RngCore;
    use assert2::assert;

    async fn echo(req: Request<Body>) -> Result<Response<Body>, Infallible> {
        assert!(req.method() == Method::POST);
        assert!(req.version() == Version::HTTP_11);
        assert!(req.uri().authority() == None);
        assert!(req.uri().path_and_query() == Some(&PathAndQuery::from_static("/echo")));

        let full_body = hyper::body::to_bytes(req.into_body()).await.unwrap();
        Ok(Response::new(full_body.into()))
    }

    fn echo_server(port: u16) -> impl Future<Output=Result<(), hyper::Error>> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));

        // A `Service` is needed for every connection, so this
        // creates one from our `hello_world` function.
        let make_svc = hyper::service::make_service_fn(|_conn| async {
            // service_fn converts our function into a `Service`
            Ok::<_, Infallible>(hyper::service::service_fn(echo))
        });

        Server::bind(&addr).serve(make_svc)
    }

    fn tls_echo_server(port: u16) -> impl Future<Output=Result<(), hyper::Error>> {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));

        // A `Service` is needed for every connection, so this
        // creates one from our `hello_world` function.
        let make_svc = hyper::service::make_service_fn(|_conn| async {
            // service_fn converts our function into a `Service`
            Ok::<_, Infallible>(hyper::service::service_fn(echo))
        });

        let server_config = crate::tls::test_server_config().unwrap();
        let acceptor: tokio_rustls::TlsAcceptor = server_config.into();
        let incoming = TlsListener::new(acceptor, AddrIncoming::bind(&addr).unwrap());

        Server::builder(incoming).serve(make_svc)
    }

    fn start_echo_server(port: u16, use_tls: bool) -> JoinHandle<Result<(), hyper::Error>> {
        if !use_tls {
            tokio::task::spawn(echo_server(port))
        } else {
            tokio::task::spawn(tls_echo_server(port))
        }
    }

    async fn start_enclave_proxy(proxy_port: u16, egress_port: u32) -> JoinHandle<()> {
        let proxy = super::EnclaveHttpProxy::bind(proxy_port).await.unwrap();
        let policy = Arc::new(crate::policy::EgressPolicy::allow_all());
        tokio::task::spawn(async move { proxy.serve(egress_port, policy).await; } )
    }

    fn start_host_proxy(egress_port: u32) -> JoinHandle<()> {
        let proxy = super::HostHttpProxy::bind(egress_port).unwrap();
        tokio::task::spawn(async move { proxy.serve().await; })
    }

    struct HttpProxyFixture {
        base_port: u16,
        host_proxy_task: JoinHandle<()>,
        enclave_proxy_task: JoinHandle<()>,
        echo_task: JoinHandle<Result<(), hyper::Error>>,
    }

    impl HttpProxyFixture {
        async fn start(base_port: u16, use_tls: bool) -> Self {
            _ = pretty_env_logger::try_init();

            return Self{
                base_port: base_port,
                enclave_proxy_task: start_enclave_proxy(base_port, base_port as u32).await,
                host_proxy_task: start_host_proxy(base_port as u32),
                echo_task: start_echo_server(base_port + 1, use_tls),
            }
        }

        fn proxy_uri(&self) -> http::Uri {
            format!("http://127.0.0.1:{}", self.base_port)
                .parse()
                .unwrap()
        }

        fn webserver_port(&self) -> u16 {
            self.base_port + 1
        }

        async fn stop(self) {
            self.echo_task.abort();
            _ = self.echo_task.await;

            self.enclave_proxy_task.abort();
            _ = self.enclave_proxy_task.await;

            self.host_proxy_task.abort();
            _ = self.host_proxy_task.await;
        }
    }

    fn random_bytes(count: usize) -> Vec<u8> {
        let mut v = vec![0u8; count];
        rand::thread_rng().fill_bytes(&mut v);
        v
    }

    #[tokio::test]
    async fn test_http_proxy() {
        let fixture = HttpProxyFixture::start(3000, false).await;

        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::http(fixture.proxy_uri().to_string()).unwrap())
            .build()
            .unwrap();

        let expected = random_bytes(128*1000);

        // 200 expected
        let resp1 = client.post(format!("http://localhost:{}/echo", fixture.webserver_port()))
            .body(expected.clone())
            .send().await.unwrap();

        let actual = resp1.bytes().await.unwrap();

        assert!(&expected == &actual);

        // Connection failure
        let resp2 = client.post(format!("http://adfadfadfadfadsfa.local/echo"))
            .body(expected.clone())
            .send().await.unwrap();

        assert!(resp2.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE);

        fixture.stop().await;
    }

    #[tokio::test]
    async fn test_https_proxy() {
        let fixture = HttpProxyFixture::start(4000, true).await;

        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::http(fixture.proxy_uri().to_string()).unwrap())
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let expected = random_bytes(128*1000);

        let resp1 = client.post(format!("https://localhost:{}/echo", fixture.webserver_port()))
            .body(expected.clone())
            .send().await.unwrap();

        let actual = resp1.bytes().await.unwrap();

        assert!(&expected == &actual);

        // Connection failure
        let resp_result = client.post(format!("https://adfadfadfadfadsfa.local/echo"))
            .body(expected.clone())
            .send().await;

        assert!(resp_result.is_err());
        assert!(resp_result.unwrap_err().is_connect());

        fixture.stop().await;
    }
}
