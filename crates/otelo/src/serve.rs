use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::Router;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::own::{self, Destination};
use crate::ui;
use otelo_api::{self as api, Api};
use otelo_host::{Collector, HostIdentity};
use otelo_storage::{BatchSender, Storage, now_unix_nanos};
use otelo_storage_sqlite::Sqlite;

#[cfg(target_os = "macos")]
const DEFAULT_DATA_DIR: &str = "/usr/local/var/otelo";
#[cfg(not(target_os = "macos"))]
const DEFAULT_DATA_DIR: &str = "/var/lib/otelo";

// An OTLP batch can hold a few hundred kilobytes, so the queue stays at tens of megabytes.
const TELEMETRY_QUEUE_BATCHES: usize = 64;

const HOST_METRICS_INTERVAL_NS: i64 = 15 * 1_000_000_000;

#[derive(clap::Args)]
pub struct ServeArgs {
    /// Address of the HTTP server
    #[arg(
        long = "listen",
        value_name = "LISTEN",
        default_value = "127.0.0.1:7070"
    )]
    api_addr: SocketAddr,

    /// Address of the OTLP receiver over HTTP
    #[arg(long, default_value = "127.0.0.1:4318")]
    otlp_http: SocketAddr,

    /// Address of the OTLP receiver over gRPC
    #[arg(long, default_value = "127.0.0.1:4317")]
    otlp_grpc: SocketAddr,

    /// Directory for the state and the telemetry, made when it is missing
    #[arg(long = "data", value_name = "DATA", default_value = DEFAULT_DATA_DIR)]
    data_dir: PathBuf,

    /// Where the daemon sends its own traces and logs: `self` for its own OTLP
    /// receiver, `off`, or the http:// URL of another OTLP/HTTP receiver
    #[arg(long, default_value = "self")]
    own_telemetry: Destination,
}

struct Listeners {
    api: TcpListener,
    otlp_http: TcpListener,
    otlp_grpc: TcpListener,
}

impl Listeners {
    async fn bind_all(args: &ServeArgs) -> anyhow::Result<Self> {
        Ok(Self {
            api: bind_listener(args.api_addr).await?,
            otlp_http: bind_listener(args.otlp_http).await?,
            otlp_grpc: bind_listener(args.otlp_grpc).await?,
        })
    }
}

pub fn run_daemon(args: ServeArgs) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("start the async runtime")?
        .block_on(async {
            let listeners = Listeners::bind_all(&args).await?;
            let host = HostIdentity::read_from_this_machine();
            let own_telemetry = own::Telemetry::spawn_exporters(
                &args.own_telemetry,
                listeners.otlp_http.local_addr()?,
                &host,
            )?;
            own::install_tracing_subscriber(own_telemetry.as_ref());
            let shutdown = CancellationToken::new();
            let signal = listen_for_shutdown_signal().context("listen for SIGTERM")?;
            tokio::spawn({
                let shutdown = shutdown.clone();
                let own_telemetry = own_telemetry.clone();
                async move {
                    signal.await;
                    tracing::info!("stopping");
                    // The receivers still run, so the daemon's last telemetry reaches them.
                    if let Some(own_telemetry) = own_telemetry {
                        let _ = tokio::task::spawn_blocking(move || {
                            own_telemetry.shut_down_exporters();
                        })
                        .await;
                    }
                    shutdown.cancel();
                }
            });
            let result = serve_until_shutdown(args, listeners, host, shutdown).await;
            if let Some(own_telemetry) = own_telemetry {
                tokio::task::spawn_blocking(move || own_telemetry.shut_down_exporters())
                    .await
                    .context("stop the exporters of the daemon's own telemetry")?;
            }
            result
        })
}

async fn serve_until_shutdown(
    args: ServeArgs,
    listeners: Listeners,
    host: HostIdentity,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(&args.data_dir)
        .with_context(|| format!("make the data directory {}", args.data_dir.display()))?;
    let storage = Sqlite::open(&args.data_dir)?;
    let (batch_sender, inbox) = otelo_storage::open_batch_channel(TELEMETRY_QUEUE_BATCHES);
    let writer = storage.spawn_writer(inbox)?;
    let storage: Arc<dyn Storage> = Arc::new(storage);
    let api = Api {
        storage: Arc::clone(&storage),
    };
    let Listeners {
        api: api_listener,
        otlp_http,
        otlp_grpc,
    } = listeners;
    tracing::info!(
        addr = %api_listener.local_addr()?,
        otlp_http = %otlp_http.local_addr()?,
        otlp_grpc = %otlp_grpc.local_addr()?,
        data = %args.data_dir.display(),
        "listening"
    );
    let serve_api = async {
        axum::serve(api_listener, build_daemon_router(api))
            .with_graceful_shutdown(shutdown.clone().cancelled_owned())
            .await
            .context("serve HTTP")
    };
    tokio::try_join!(
        serve_api,
        otelo_otlp::serve_http(otlp_http, batch_sender.clone(), shutdown.clone()),
        otelo_otlp::serve_grpc(otlp_grpc, batch_sender.clone(), shutdown.clone()),
        collect_host_metrics(host, storage, batch_sender.clone(), shutdown.clone()),
    )?;
    // The writer ends once the last sender is gone.
    drop(batch_sender);
    tokio::task::spawn_blocking(|| writer.join())
        .await
        .context("wait for the telemetry writer")??;
    tracing::info!("stopped");
    Ok(())
}

// The readers of the collector block, and the runtime of a machine with one CPU has one worker
// thread, so every read runs on a thread that may block.
async fn collect_host_metrics(
    host: HostIdentity,
    storage: Arc<dyn Storage>,
    batch_sender: BatchSender,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let new_collector = tokio::task::spawn_blocking(move || Collector::new(host))
        .await
        .context("start the host collector")?;
    let mut collector = match new_collector {
        Ok(collector) => collector,
        Err(error) => {
            tracing::error!("no host metrics: {error:#}");
            return Ok(());
        }
    };
    // The first reading comes at once, and every later one on a multiple of the interval, so
    // the points of all series line up.
    let mut recorded_at = now_unix_nanos();
    loop {
        let storage = Arc::clone(&storage);
        let (collector_after_reading, batch) = tokio::task::spawn_blocking(move || {
            let storage_size = storage
                .size()
                .inspect_err(|error| tracing::warn!("read the size of the storage: {error}"))
                .ok();
            let batch = collector.collect_batch(recorded_at, storage_size);
            (collector, batch)
        })
        .await
        .context("collect the host metrics")?;
        collector = collector_after_reading;
        match batch {
            // The writer reports the batches a full channel dropped.
            Ok(batch) => drop(batch_sender.send_batch(batch)),
            Err(error) => tracing::warn!("collect the host metrics: {error:#}"),
        }
        // A reading that ran past a tick skips it.
        let now = now_unix_nanos();
        recorded_at = (now.div_euclid(HOST_METRICS_INTERVAL_NS) + 1) * HOST_METRICS_INTERVAL_NS;
        let time_until_tick = Duration::from_nanos(u64::try_from(recorded_at - now).unwrap_or(0));
        tokio::select! {
            () = shutdown.cancelled() => return Ok(()),
            () = tokio::time::sleep(time_until_tick) => {}
        }
    }
}

async fn bind_listener(addr: SocketAddr) -> anyhow::Result<TcpListener> {
    TcpListener::bind(addr)
        .await
        .with_context(|| format!("listen on {addr}"))
}

fn build_daemon_router(api: Api) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(api::build_router(api))
        .fallback(ui::serve_ui)
}

fn listen_for_shutdown_signal() -> io::Result<impl Future<Output = ()>> {
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    Ok(async move {
        #[cfg(unix)]
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    })
}
