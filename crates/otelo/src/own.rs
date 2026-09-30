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
use otelo_host::HostIdentity;
use tracing_subscriber::filter::{FilterExt, filter_fn};
use tracing_subscriber::layer::Filter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

// Exporting the events of these crates could export without end.
const EXPORTER_TARGETS: [&str; 6] = [
    "opentelemetry",
    "reqwest",
    "hyper",
    "hyper_util",
    "h2",
    "tower",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    OwnReceiver,
    Off,
    OtherReceiver(String),
}

impl FromStr for Destination {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "self" => Ok(Self::OwnReceiver),
            "off" => Ok(Self::Off),
            url if url.starts_with("http://") => {
                Ok(Self::OtherReceiver(url.trim_end_matches('/').into()))
            }
            _ => Err(format!(
                "{text:?} has to be self, off, or an http:// URL of an OTLP/HTTP receiver"
            )),
        }
    }
}

impl fmt::Display for Destination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnReceiver => f.write_str("self"),
            Self::Off => f.write_str("off"),
            Self::OtherReceiver(url) => f.write_str(url),
        }
    }
}

#[derive(Clone)]
pub struct Telemetry {
    tracer_provider: SdkTracerProvider,
    logger_provider: SdkLoggerProvider,
    // Cleared at shutdown: the stopped exporters would warn about each later span and event.
    exporting: Arc<AtomicBool>,
}

impl Telemetry {
    pub fn spawn_exporters(
        destination: &Destination,
        own_receiver_addr: SocketAddr,
        host: &HostIdentity,
    ) -> anyhow::Result<Option<Self>> {
        let receiver_url = match destination {
            Destination::Off => return Ok(None),
            Destination::OwnReceiver => format!("http://{own_receiver_addr}"),
            Destination::OtherReceiver(url) => url.clone(),
        };
        let resource = Resource::builder()
            .with_service_name("otelo")
            .with_attribute(opentelemetry::KeyValue::new(
                "service.version",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_attributes(
                host.attributes()
                    .into_iter()
                    .map(|(key, value)| opentelemetry::KeyValue::new(key, value)),
            )
            .build();
        let span_exporter = SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(format!("{receiver_url}/v1/traces"))
            .build()
            .context("build the span exporter")?;
        let log_exporter = LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(format!("{receiver_url}/v1/logs"))
            .build()
            .context("build the log exporter")?;
        Ok(Some(Self {
            tracer_provider: SdkTracerProvider::builder()
                .with_resource(resource.clone())
                .with_batch_exporter(span_exporter)
                .build(),
            logger_provider: SdkLoggerProvider::builder()
                .with_resource(resource)
                .with_batch_exporter(log_exporter)
                .build(),
            exporting: Arc::new(AtomicBool::new(true)),
        }))
    }

    pub fn shut_down_exporters(&self) {
        self.exporting.store(false, Ordering::Relaxed);
        for (signal, result) in [
            ("traces", self.tracer_provider.shutdown()),
            ("logs", self.logger_provider.shutdown()),
        ] {
            match result {
                Ok(()) | Err(OTelSdkError::AlreadyShutdown) => {}
                Err(error) => tracing::warn!("export the daemon's own {signal}: {error}"),
            }
        }
    }
}

pub fn install_tracing_subscriber(own_telemetry: Option<&Telemetry>) {
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(io::stderr)
        .with_ansi(io::stderr().is_terminal())
        .with_filter(build_env_filter(&[]));
    // A span on the path from the OTLP receiver to the day files would export itself forever.
    let trace_export_layer = own_telemetry.map(|own_telemetry| {
        tracing_opentelemetry::layer()
            .with_tracer(own_telemetry.tracer_provider.tracer("otelo"))
            .with_tracked_inactivity(false)
            .with_target(false)
            .with_filter(own_telemetry.build_exporter_filter())
    });
    let log_export_layer = own_telemetry.map(|own_telemetry| {
        OpenTelemetryTracingBridge::new(&own_telemetry.logger_provider)
            .with_filter(own_telemetry.build_exporter_filter())
    });
    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(trace_export_layer)
        .with(log_export_layer)
        .init();
}

impl Telemetry {
    fn build_exporter_filter<S>(&self) -> impl Filter<S> + use<S> {
        let exporting = Arc::clone(&self.exporting);
        let while_exporting = filter_fn(move |_| exporting.load(Ordering::Relaxed));
        build_env_filter(&EXPORTER_TARGETS).and(while_exporting)
    }
}

fn build_env_filter(muted_targets: &[&str]) -> EnvFilter {
    let mut filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    for target in muted_targets {
        filter = filter.add_directive(
            format!("{target}=off")
                .parse()
                .expect("a target directive parses"),
        );
    }
    filter
}
