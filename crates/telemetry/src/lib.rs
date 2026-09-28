//! The daily SQLite files that hold the logs, traces, and metrics.
//!
//! One writer thread owns every write. Sources send it batches through a
//! [`Sender`], and a [`Reader`] attaches the day files that a time range covers.
//! Every timestamp is in unix nanoseconds.

mod catalog;
mod day;
mod histogram;
mod indexes;
pub mod query;
mod reader;
mod writer;

use serde_json::{Map, Value};

pub use day::{Day, now};
pub use histogram::{Distribution, Histogram, Merger};
pub use indexes::{IndexedKey, Indexes};
pub use reader::{Reader, timed_out};
pub use writer::{Config, Inbox, Sender, Writer, channel};

/// Everything one source sends in one go. A full channel drops all of it.
pub type Batch = Vec<Records>;

/// The telemetry of one resource.
#[derive(Clone, Debug)]
pub struct Records {
    pub resource: Resource,
    pub logs: Vec<Log>,
    pub spans: Vec<Span>,
    pub metrics: Vec<Metric>,
}

/// What emitted the telemetry: the OpenTelemetry resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    /// The `service.name` attribute, or the app name for a service log.
    pub service: String,
    pub attributes: Map<String, Value>,
}

#[derive(Clone, Debug)]
pub struct Log {
    pub ts: i64,
    /// The OpenTelemetry severity number.
    pub severity: i32,
    pub body: String,
    pub trace_id: Option<[u8; 16]>,
    pub span_id: Option<[u8; 8]>,
    pub attributes: Map<String, Value>,
    /// `otlp`, or the service log source that read the line.
    pub source: &'static str,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub parent_span_id: Option<[u8; 8]>,
    pub name: String,
    /// The OpenTelemetry span kind.
    pub kind: i32,
    pub start_ts: i64,
    pub duration_ns: i64,
    /// The OpenTelemetry status code.
    pub status: i32,
    pub attributes: Map<String, Value>,
    pub events: Vec<Value>,
}

/// The points of one series.
#[derive(Clone, Debug)]
pub struct Metric {
    pub name: String,
    pub kind: MetricKind,
    pub unit: String,
    pub labels: Map<String, Value>,
    pub points: Vec<Point>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricKind {
    Gauge,
    Sum,
    Histogram,
}

impl MetricKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gauge => "gauge",
            Self::Sum => "sum",
            Self::Histogram => "histogram",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Point {
    pub ts: i64,
    pub value: f64,
    /// The buckets of a histogram point. `None` for a gauge or a sum.
    pub histogram: Option<Histogram>,
}
