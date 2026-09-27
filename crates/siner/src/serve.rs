use std::io::{self, IsTerminal};
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Context;
use axum::Router;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[cfg(target_os = "macos")]
const DEFAULT_DATA_DIR: &str = "/usr/local/var/siner";
#[cfg(not(target_os = "macos"))]
const DEFAULT_DATA_DIR: &str = "/var/lib/siner";

#[derive(clap::Args)]
pub struct Args {
    /// Address of the HTTP server
    #[arg(long, default_value = "127.0.0.1:7070")]
    listen: SocketAddr,

    /// Directory for the state and the telemetry, made when it is missing
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data: PathBuf,
}

pub fn main(args: Args) -> anyhow::Result<()> {
    init_logging();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("start the async runtime")?
        .block_on(async {
            let shutdown = CancellationToken::new();
            let signal = shutdown_signal().context("listen for SIGTERM")?;
            tokio::spawn({
                let shutdown = shutdown.clone();
                async move {
                    signal.await;
                    tracing::info!("stopping");
                    shutdown.cancel();
                }
            });
            run(args, shutdown).await
        })
}

/// Logs go to stderr, where journald picks them up when systemd runs the daemon.
fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_writer(io::stderr)
        .with_ansi(io::stderr().is_terminal())
        .init();
}

/// Runs the daemon until `shutdown` is cancelled. Every source stops on the
/// same token, and the writer flushes after the sources end.
async fn run(args: Args, shutdown: CancellationToken) -> anyhow::Result<()> {
    std::fs::create_dir_all(&args.data)
        .with_context(|| format!("make the data directory {}", args.data.display()))?;
    let listener = TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("listen on {}", args.listen))?;
    tracing::info!(
        addr = %listener.local_addr()?,
        data = %args.data.display(),
        "listening"
    );
    axum::serve(listener, router())
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await
        .context("serve HTTP")?;
    tracing::info!("stopped");
    Ok(())
}

fn router() -> Router {
    Router::new().route("/health", get(|| async { "ok" }))
}

/// Resolves on Ctrl-C, and on SIGTERM where there is one.
fn shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    #[cfg(unix)]
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    Ok(async move {
        #[cfg(unix)]
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    })
}
