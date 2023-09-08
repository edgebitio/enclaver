use anyhow::{anyhow, Result};
use futures_util::stream::StreamExt;
use log::info;
use std::future::Future;
use std::path::PathBuf;
use tokio::io::AsyncRead;
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::codec::{FramedRead, LinesCodec};

const LOG_LINE_MAX_LEN: usize = 4 * 1024;

#[cfg(feature = "tracing")]
#[macro_export]
macro_rules! spawn {
    ($name:expr, $body:expr) => {{
        tokio::task::Builder::new().name($name).spawn($body)
    }};
}

#[cfg(not(feature = "tracing"))]
#[macro_export]
macro_rules! spawn {
    ($name:expr, $body:expr) => {{
        Result::<_, anyhow::Error>::Ok(tokio::task::spawn($body))
    }};
}

pub use spawn;

pub fn init_logging() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();
}

pub trait StringablePathExt {
    fn must_to_str(&self) -> Result<&str>;
    fn must_to_string(&self) -> Result<String>;
}

impl StringablePathExt for PathBuf {
    fn must_to_str(&self) -> Result<&str> {
        self.to_str()
            .ok_or_else(|| anyhow!("filename contains non-UTF-8 characters"))
    }

    fn must_to_string(&self) -> Result<String> {
        self.to_str()
            .ok_or_else(|| anyhow!("filename contains non-UTF-8 characters"))
            .map(|s| s.to_string())
    }
}

pub async fn log_lines_from_stream<S>(target: &str, stream: S) -> Result<()>
where
    S: AsyncRead + Unpin,
{
    let mut framed = FramedRead::new(stream, LinesCodec::new_with_max_length(LOG_LINE_MAX_LEN));

    while let Some(line_res) = framed.next().await {
        match line_res {
            Ok(line) => info!(target: target, "{line}"),
            Err(e) => info!(target: target, "error reading log stream: {e}"),
        }
    }

    Ok(())
}

pub async fn register_shutdown_signal_handler() -> Result<impl Future> {
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    spawn!("signal handler", async move {
        tokio::select! {
            _ = sigint.recv() => (),
            _ = sigterm.recv() => (),
        }
    })
    .map_err(Into::into)
}
