use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, ensure};
use axum::routing::get;
use axum::{Router, middleware};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::own::{self, Destination};
use crate::ui;
use otelo_api::{self as api, Api};
use otelo_host::{Collector, HostIdentity};
use otelo_indexed_storage::{PipelineMeters, Storage, StorageSize, now_unix_nanos};
use otelo_indexed_storage_sqlite::{FrameMapper, Sqlite};
use otelo_journal::Journal;
use otelo_journal_files::{JournalFiles, OpenedJournal};
use otelo_otlp::Intake;
use otelo_state::StateFile;

#[cfg(target_os = "macos")]
pub const DEFAULT_DATA_DIR: &str = "/usr/local/var/otelo";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_DATA_DIR: &str = "/var/lib/otelo";

pub const JOURNAL_DIRECTORY_NAME: &str = "journal";

const HOST_METRICS_INTERVAL_NS: i64 = 15 * 1_000_000_000;

// glibc gives each thread that allocates at the same time its own arena, and an arena keeps
// the peak of the queries it served, so every blocking thread adds that peak to the RSS.
const MAX_BLOCKING_THREADS: usize = 2;

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

    /// Serve the API without a password, for local development; the HTTP server
    /// has to listen on a loopback address
    #[arg(long)]
    unsafe_no_auth: bool,
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
    ensure!(
        !args.unsafe_no_auth || args.api_addr.ip().is_loopback(),
        "--unsafe-no-auth needs a loopback address for --listen, not {}",
        args.api_addr
    );
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(MAX_BLOCKING_THREADS)
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
            own::log_panics_as_errors();
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
    let state = Arc::new(StateFile::open(&args.data_dir)?);
    if args.unsafe_no_auth {
        tracing::warn!("the API needs no password: --unsafe-no-auth");
    } else {
        ensure!(
            state.has_password()?,
            "{} has no password; `otelo init --data {}` makes one",
            args.data_dir.display(),
            args.data_dir.display()
        );
    }
    let storage = Sqlite::open(&args.data_dir, state.indexed_attributes()?)?;
    let OpenedJournal {
        journal,
        threads: journal_threads,
        synced_ends,
    } = JournalFiles::open(otelo_journal_files::Config::new(
        args.data_dir.join(JOURNAL_DIRECTORY_NAME),
    ))?;
    let journal: Arc<dyn Journal> = journal;
    let meters = Arc::new(PipelineMeters::default());
    let map_frame: FrameMapper = Arc::new(otelo_otlp::map::map_journal_frame);
    let indexer = storage.spawn_indexer(
        Arc::clone(&journal),
        synced_ends,
        map_frame,
        Arc::clone(&meters),
    )?;
    let intake = Intake::new(Arc::clone(&journal), Arc::clone(&meters));
    let storage: Arc<dyn Storage> = Arc::new(storage);
    let api = Api::new(Arc::clone(&storage), Arc::clone(&state));
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
        axum::serve(api_listener, build_daemon_router(api, args.unsafe_no_auth))
            .with_graceful_shutdown(shutdown.clone().cancelled_owned())
            .await
            .context("serve HTTP")
    };
    tokio::try_join!(
        serve_api,
        otelo_otlp::serve_http(otlp_http, intake.clone(), shutdown.clone()),
        otelo_otlp::serve_grpc(otlp_grpc, intake.clone(), shutdown.clone()),
        collect_host_metrics(
            host,
            CollectorSources {
                storage,
                state,
                journal,
                meters,
            },
            intake,
            shutdown.clone(),
        ),
    )?;
    // The indexer ends once the sync threads of the journal are gone.
    tokio::task::spawn_blocking(|| journal_threads.stop_and_join())
        .await
        .context("wait for the journal")??;
    tokio::task::spawn_blocking(|| indexer.join())
        .await
        .context("wait for the telemetry indexer")??;
    tracing::info!("stopped");
    Ok(())
}

// The readers of the collector block, and the runtime of a machine with one CPU has one worker
// thread, so every read runs on a thread that may block.
struct CollectorSources {
    storage: Arc<dyn Storage>,
    state: Arc<StateFile>,
    journal: Arc<dyn Journal>,
    meters: Arc<PipelineMeters>,
}

async fn collect_host_metrics(
    host: HostIdentity,
    sources: CollectorSources,
    intake: Intake,
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
        let storage = Arc::clone(&sources.storage);
        let state = Arc::clone(&sources.state);
        let journal = Arc::clone(&sources.journal);
        let pipeline = sources.meters.read_pipeline();
        let (collector_after_reading, request) = tokio::task::spawn_blocking(move || {
            let storage_size = read_storage_size(storage.as_ref(), &state, journal.as_ref())
                .inspect_err(|error| tracing::warn!("read the size of the storage: {error:#}"))
                .ok();
            let request = collector.collect_request(recorded_at, storage_size, Some(&pipeline));
            (collector, request)
        })
        .await
        .context("collect the host metrics")?;
        collector = collector_after_reading;
        match request {
            // The intake logs and counts a request the journal could not keep.
            Ok(request) => drop(intake.accept_export(request).await),
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

fn read_storage_size(
    storage: &dyn Storage,
    state: &StateFile,
    journal: &dyn Journal,
) -> anyhow::Result<StorageSize> {
    Ok(StorageSize {
        journal_bytes: journal.size_in_bytes()?,
        telemetry_bytes: storage.size_in_bytes()?,
        state_bytes: state.size_in_bytes()?,
    })
}

async fn bind_listener(addr: SocketAddr) -> anyhow::Result<TcpListener> {
    TcpListener::bind(addr)
        .await
        .with_context(|| format!("listen on {addr}"))
}

fn build_daemon_router(api: Api, unsafe_no_auth: bool) -> Router {
    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(api::build_router(api.clone()))
        .fallback(ui::serve_ui);
    if unsafe_no_auth {
        router
    } else {
        router.layer(middleware::from_fn_with_state(api, api::require_password))
    }
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
