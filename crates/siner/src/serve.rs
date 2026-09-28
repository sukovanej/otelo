use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use axum::routing::get;
use siner_telemetry::Indexes;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::api::{self, Api};
use crate::own::{self, Destination};
use crate::state::State;

#[cfg(target_os = "macos")]
const DEFAULT_DATA_DIR: &str = "/usr/local/var/siner";
#[cfg(not(target_os = "macos"))]
const DEFAULT_DATA_DIR: &str = "/var/lib/siner";

/// Batches the sources can queue for the telemetry writer. An OTLP batch can
/// hold a few hundred kilobytes, so this bounds the queue to tens of megabytes.
const TELEMETRY_CAPACITY: usize = 64;

#[derive(clap::Args)]
pub struct Args {
    /// Address of the HTTP server
    #[arg(long, default_value = "127.0.0.1:7070")]
    listen: SocketAddr,

    /// Address of the OTLP receiver over HTTP
    #[arg(long, default_value = "127.0.0.1:4318")]
    otlp_http: SocketAddr,

    /// Address of the OTLP receiver over gRPC
    #[arg(long, default_value = "127.0.0.1:4317")]
    otlp_grpc: SocketAddr,

    /// Directory for the state and the telemetry, made when it is missing
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data: PathBuf,

    /// Where the daemon sends its own traces and logs: `self` for its own OTLP
    /// receiver, `off`, or the http:// URL of another OTLP/HTTP receiver
    #[arg(long, default_value = "self")]
    own_telemetry: Destination,
}

/// The listeners of the daemon, bound before anything else starts.
struct Listeners {
    api: TcpListener,
    otlp_http: TcpListener,
    otlp_grpc: TcpListener,
}

impl Listeners {
    async fn bind(args: &Args) -> anyhow::Result<Self> {
        Ok(Self {
            api: bind(args.listen).await?,
            otlp_http: bind(args.otlp_http).await?,
            otlp_grpc: bind(args.otlp_grpc).await?,
        })
    }
}

/// Runs the daemon until SIGTERM or Ctrl-C.
///
/// # Errors
///
/// When the data directory, the state file, the telemetry writer, the
/// exporters of its own telemetry, or a listener cannot start, or the server
/// fails.
pub fn main(args: Args) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("start the async runtime")?
        .block_on(async {
            let listeners = Listeners::bind(&args).await?;
            let own =
                own::Telemetry::start(&args.own_telemetry, listeners.otlp_http.local_addr()?)?;
            own::init_logging(own.as_ref());
            let shutdown = CancellationToken::new();
            let signal = shutdown_signal().context("listen for SIGTERM")?;
            tokio::spawn({
                let shutdown = shutdown.clone();
                let own = own.clone();
                async move {
                    signal.await;
                    tracing::info!("stopping");
                    // The receivers still run, so the last of the daemon's
                    // own telemetry reaches them.
                    if let Some(own) = own {
                        let _ = tokio::task::spawn_blocking(move || own.shutdown()).await;
                    }
                    shutdown.cancel();
                }
            });
            let result = run(args, listeners, shutdown).await;
            if let Some(own) = own {
                tokio::task::spawn_blocking(move || own.shutdown())
                    .await
                    .context("stop the exporters of the daemon's own telemetry")?;
            }
            result
        })
}

/// Runs the daemon until `shutdown` is cancelled. Every server and source
/// stops on the same token, and the writer flushes after the sources end.
async fn run(args: Args, listeners: Listeners, shutdown: CancellationToken) -> anyhow::Result<()> {
    std::fs::create_dir_all(&args.data)
        .with_context(|| format!("make the data directory {}", args.data.display()))?;
    let state = State::open(&args.data)?;
    let (telemetry, inbox) = siner_telemetry::channel(TELEMETRY_CAPACITY);
    let mut config = siner_telemetry::Config::new(args.data.join("telemetry"));
    config.indexes = Indexes::new(state.indexes()?);
    let api = Api {
        dir: config.dir.clone(),
        retention_days: config.retention_days,
        indexes: config.indexes.clone(),
        state: Arc::new(state),
    };
    let writer = siner_telemetry::Writer::spawn(config, inbox)?;
    let Listeners {
        api: listener,
        otlp_http,
        otlp_grpc,
    } = listeners;
    tracing::info!(
        addr = %listener.local_addr()?,
        otlp_http = %otlp_http.local_addr()?,
        otlp_grpc = %otlp_grpc.local_addr()?,
        data = %args.data.display(),
        "listening"
    );
    let api = async {
        axum::serve(listener, router(api))
            .with_graceful_shutdown(shutdown.clone().cancelled_owned())
            .await
            .context("serve HTTP")
    };
    tokio::try_join!(
        api,
        siner_otlp::serve_http(otlp_http, telemetry.clone(), shutdown.clone()),
        siner_otlp::serve_grpc(otlp_grpc, telemetry.clone(), shutdown.clone()),
    )?;
    // The writer ends once the last sender is gone.
    drop(telemetry);
    tokio::task::spawn_blocking(|| writer.join())
        .await
        .context("wait for the telemetry writer")??;
    tracing::info!("stopped");
    Ok(())
}

async fn bind(addr: SocketAddr) -> anyhow::Result<TcpListener> {
    TcpListener::bind(addr)
        .await
        .with_context(|| format!("listen on {addr}"))
}

fn router(api: Api) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(api::router(api))
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
