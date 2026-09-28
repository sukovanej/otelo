//! The daemon's own logs and traces. They go to stderr, where journald picks
//! them up, and over OTLP/HTTP to a receiver: by default the daemon's own, so
//! siner can be tried and debugged on itself.
//!
//! Nothing on the path from the OTLP receiver to the day files opens a span.
//! A span there would make each export of the daemon's telemetry export
//! another one, forever. For the same reason the exporter's own crates never
//! reach the exporter.

use std::fmt;
use std::io::{self, IsTerminal};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context;
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkError;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::filter::{FilterExt, filter_fn};
use tracing_subscriber::layer::Filter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// The crates that export the daemon's telemetry, or that it exports through.
/// Their events stay on stderr: exporting them could export without end.
const EXPORTER_TARGETS: [&str; 6] = [
    "opentelemetry",
    "reqwest",
    "hyper",
    "hyper_util",
    "h2",
    "tower",
];

/// Where the daemon sends its own logs and traces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    /// The daemon's own OTLP receiver over HTTP.
    Receiver,
    /// Only stderr.
    Off,
    /// The base URL of another OTLP/HTTP receiver, such as `http://127.0.0.1:4318`.
    Url(String),
}

impl FromStr for Destination {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "self" => Ok(Self::Receiver),
            "off" => Ok(Self::Off),
            url if url.starts_with("http://") => Ok(Self::Url(url.trim_end_matches('/').into())),
            _ => Err(format!(
                "{text:?} has to be self, off, or an http:// URL of an OTLP/HTTP receiver"
            )),
        }
    }
}

impl fmt::Display for Destination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Receiver => f.write_str("self"),
            Self::Off => f.write_str("off"),
            Self::Url(url) => f.write_str(url),
        }
    }
}

/// The exporters of the daemon's own telemetry.
#[derive(Clone)]
pub struct Telemetry {
    tracer: SdkTracerProvider,
    logger: SdkLoggerProvider,
    /// Cleared at shutdown, so later spans and events do not reach the
    /// stopped exporters, which would warn about each.
    exporting: Arc<AtomicBool>,
}

impl Telemetry {
    /// Starts the exporters for `destination`, with `receiver` as the address
    /// of the daemon's own OTLP/HTTP receiver. `None` for [`Destination::Off`].
    ///
    /// # Errors
    ///
    /// When an exporter cannot be built.
    pub fn start(destination: &Destination, receiver: SocketAddr) -> anyhow::Result<Option<Self>> {
        let base = match destination {
            Destination::Off => return Ok(None),
            Destination::Receiver => format!("http://{receiver}"),
            Destination::Url(url) => url.clone(),
        };
        let resource = Resource::builder()
            .with_service_name("siner")
            .with_attribute(opentelemetry::KeyValue::new(
                "service.version",
                env!("CARGO_PKG_VERSION"),
            ))
            .build();
        let spans = SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(format!("{base}/v1/traces"))
            .build()
            .context("build the span exporter")?;
        let logs = LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(format!("{base}/v1/logs"))
            .build()
            .context("build the log exporter")?;
        Ok(Some(Self {
            tracer: SdkTracerProvider::builder()
                .with_resource(resource.clone())
                .with_batch_exporter(spans)
                .build(),
            logger: SdkLoggerProvider::builder()
                .with_resource(resource)
                .with_batch_exporter(logs)
                .build(),
            exporting: Arc::new(AtomicBool::new(true)),
        }))
    }

    /// Exports what is queued and stops. Later spans and events reach only
    /// stderr. It blocks until the receiver answers or the export times out.
    pub fn shutdown(&self) {
        self.exporting.store(false, Ordering::Relaxed);
        for (signal, result) in [
            ("traces", self.tracer.shutdown()),
            ("logs", self.logger.shutdown()),
        ] {
            match result {
                Ok(()) | Err(OTelSdkError::AlreadyShutdown) => {}
                Err(error) => tracing::warn!("export the daemon's own {signal}: {error}"),
            }
        }
    }
}

/// Sends the events of the daemon to stderr, and its spans and events to
/// `own` when there is one. `RUST_LOG` picks the level, `info` by default.
pub fn init_logging(own: Option<&Telemetry>) {
    let stderr = tracing_subscriber::fmt::layer()
        .with_writer(io::stderr)
        .with_ansi(io::stderr().is_terminal())
        .with_filter(filter(&[]));
    let traces = own.map(|own| {
        tracing_opentelemetry::layer()
            .with_tracer(own.tracer.tracer("siner"))
            .with_tracked_inactivity(false)
            .with_target(false)
            .with_filter(own.filter())
    });
    let logs =
        own.map(|own| OpenTelemetryTracingBridge::new(&own.logger).with_filter(own.filter()));
    tracing_subscriber::registry()
        .with(stderr)
        .with(traces)
        .with(logs)
        .init();
}

impl Telemetry {
    /// What the exporters take: what stderr takes, except the events of the
    /// exporter's crates, until shutdown.
    fn filter<S>(&self) -> impl Filter<S> + use<S> {
        let exporting = Arc::clone(&self.exporting);
        let open = filter_fn(move |_| exporting.load(Ordering::Relaxed));
        filter(&EXPORTER_TARGETS).and(open)
    }
}

/// The level of `RUST_LOG`, or `info`, with `off` for the `muted` targets.
fn filter(muted: &[&str]) -> EnvFilter {
    let mut filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    for target in muted {
        filter = filter.add_directive(
            format!("{target}=off")
                .parse()
                .expect("a target directive parses"),
        );
    }
    filter
}
