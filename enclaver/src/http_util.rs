use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use hyper::{server::conn::AddrIncoming, Body, Request, Response, Server, StatusCode};

#[async_trait]
pub trait HttpHandler {
    async fn handle(&self, req: Request<Body>) -> Result<Response<Body>>;
}

pub struct HttpServer {
    incoming: AddrIncoming,
}

impl HttpServer {
    pub fn bind(listen_port: u16) -> Result<Self> {
        let listen_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, listen_port));
        let incoming = AddrIncoming::bind(&listen_addr)?;
        Ok(Self { incoming })
    }

    pub async fn serve<H: HttpHandler + Send + Sync + 'static>(self, handler: H) -> Result<()> {
        let handler = Arc::new(handler);

        // thanks https://www.fpcomplete.com/blog/ownership-puzzle-rust-async-hyper/
        let make_svc = hyper::service::make_service_fn(move |_conn| {
            // service_fn converts our function into a `Service`
            let handler = handler.clone();
            async {
                Ok::<_, Infallible>(hyper::service::service_fn(move |req: Request<Body>| {
                    let handler = handler.clone();
                    async move {
                        let resp = handler
                            .handle(req)
                            .await
                            .unwrap_or_else(|err| internal_srv_err(err.to_string()));

                        Result::<_, Infallible>::Ok(resp)
                    }
                }))
            }
        });

        Server::builder(self.incoming).serve(make_svc).await?;
        Ok(())
    }
}

pub fn internal_srv_err(msg: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(msg))
        .unwrap()
}

pub fn bad_request(msg: String) -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from(msg))
        .unwrap()
}

pub fn method_not_allowed() -> Response<Body> {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Body::empty())
        .unwrap()
}

pub fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}
