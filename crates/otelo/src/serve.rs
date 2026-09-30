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
use otelo_storage::{Sender, Storage, now_unix_nanos};
use otelo_storage_sqlite::Sqlite;

#[cfg(target_os = "macos")]
const DEFAULT_DATA_DIR: &str = "/usr/local/var/otelo";
#[cfg(not(target_os = "macos"))]
const DEFAULT_DATA_DIR: &str = "/var/lib/otelo";

// An OTLP batch can hold a few hundred kilobytes, so the queue stays at tens of megabytes.
const TELEMETRY_QUEUE_BATCHES: usize = 64;

const HOST_METRICS_INTERVAL_NS: i64 = 15 * 1_000_000_000;

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

pub fn main(args: Args) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("start the async runtime")?
        .block_on(async {
            let listeners = Listeners::bind(&args).await?;
            let host = HostIdentity::of_this_machine();
            let own = own::Telemetry::start(
                &args.own_telemetry,
                listeners.otlp_http.local_addr()?,
                &host,
            )?;
            own::init_logging(own.as_ref());
            let shutdown = CancellationToken::new();
            let signal = shutdown_signal().context("listen for SIGTERM")?;
            tokio::spawn({
                let shutdown = shutdown.clone();
                let own = own.clone();
                async move {
                    signal.await;
                    tracing::info!("stopping");
                    // The receivers still run, so the daemon's last telemetry reaches them.
                    if let Some(own) = own {
                        let _ = tokio::task::spawn_blocking(move || own.shutdown()).await;
                    }
                    shutdown.cancel();
                }
            });
            let result = run_daemon(args, listeners, host, shutdown).await;
            if let Some(own) = own {
                tokio::task::spawn_blocking(move || own.shutdown())
                    .await
                    .context("stop the exporters of the daemon's own telemetry")?;
            }
            result
        })
}

async fn run_daemon(
    args: Args,
    listeners: Listeners,
    host: HostIdentity,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(&args.data)
        .with_context(|| format!("make the data directory {}", args.data.display()))?;
    let storage = Sqlite::open(&args.data)?;
    let (telemetry, inbox) = otelo_storage::batch_channel(TELEMETRY_QUEUE_BATCHES);
    let writer = storage.spawn_writer(inbox)?;
    let storage: Arc<dyn Storage> = Arc::new(storage);
    let api = Api {
        storage: Arc::clone(&storage),
    };
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
        otelo_otlp::serve_http(otlp_http, telemetry.clone(), shutdown.clone()),
        otelo_otlp::serve_grpc(otlp_grpc, telemetry.clone(), shutdown.clone()),
        collect_host_metrics(host, storage, telemetry.clone(), shutdown.clone()),
    )?;
    // The writer ends once the last sender is gone.
    drop(telemetry);
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
    telemetry: Sender,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let started = tokio::task::spawn_blocking(move || Collector::of_host(host))
        .await
        .context("start the host collector")?;
    let mut collector = match started {
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
            Ok(batch) => drop(telemetry.send(batch)),
            Err(error) => tracing::warn!("collect the host metrics: {error:#}"),
        }
        // A reading that ran past a tick skips it.
        let now = now_unix_nanos();
        recorded_at = (now.div_euclid(HOST_METRICS_INTERVAL_NS) + 1) * HOST_METRICS_INTERVAL_NS;
        let until_tick = Duration::from_nanos(u64::try_from(recorded_at - now).unwrap_or(0));
        tokio::select! {
            () = shutdown.cancelled() => return Ok(()),
            () = tokio::time::sleep(until_tick) => {}
        }
    }
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
        .fallback(ui::serve)
}

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
