use std::sync::Arc;
use log::{info, error};
use anyhow::{Result};
use rustls::ServerConfig;
use tokio_vsock::{VsockListener, VsockStream};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;
use futures::{Stream, StreamExt};

pub type TlsVsock = TlsStream<VsockStream>;

// Listen on a vsock with the given port.
// Returns a Stream of connected sockets.
pub fn serve(port: u32) -> Result<impl Stream<Item=VsockStream>> {
    let listener = VsockListener::bind(super::VMADDR_CID_ANY, port)?;

    info!("Listening on vsock port {}", port);
    let stream = listener.incoming()
        .filter_map(move |result| {
            async move {
                match result {
                    Ok(vsock) => {
                        info!("Connection accepted");
                        Some(vsock)
                    },

                    Err(e) => {
                        error!("Failed to accept a vsock: {}", e);
                        None
                    }
                }
            }
        });

    Ok(stream)
}


// Listen on a vsock with the given port for TLS connections.
// Returns a Stream of TLS connected sockets.
pub fn tls_serve(port: u32, tls_config: Arc<ServerConfig>) -> Result<impl Stream<Item=TlsVsock>> {
    let acceptor = TlsAcceptor::from(tls_config);
    let listener = VsockListener::bind(super::VMADDR_CID_ANY, port)?;

    info!("Listening on TLS vsock port {}", port);
    let stream = listener.incoming()
        .filter_map(move |result| {
            let acceptor = acceptor.clone();
            async move {
                match result {
                    Ok(vsock) => {
                        info!("Connection accepted");
                        match acceptor.accept(vsock).await {
                            Ok(vsock) => Some(vsock),
                            Err(e) => {
                                error!("TLS handshake failed: {}", e);
                                None
                            }
                        }
                    },

                    Err(e) => {
                        error!("Failed to accept a vsock: {}", e);
                        None
                    }
                }
            }
        });

    Ok(stream)
}
