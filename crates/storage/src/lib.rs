mod attributes;
mod channel;
mod error;
mod histogram;
mod indexes;
mod otel;
pub mod query;
mod range;
mod storage;

pub use attributes::{AttributeValue, Attributes, SpanEvent};
pub use channel::{Inbox, Sender, batch_channel};
pub use error::{Error, Result};
pub use histogram::{Distribution, Histogram, Merger};
pub use indexes::{IndexedAttribute, IndexedSignal};
pub use otel::{Severity, SpanId, SpanKind, SpanStatus, TraceId};
pub use range::TimeRange;
pub use storage::{RangeQueries, Storage};

#[must_use]
pub fn now_unix_nanos() -> i64 {
    i64::try_from(jiff::Timestamp::now().as_nanosecond()).expect("now fits an i64 until 2262")
}

pub type Batch = Vec<Records>;

#[derive(Clone, Debug)]
pub struct Records {
    pub resource: Resource,
    pub logs: Vec<Log>,
    pub spans: Vec<Span>,
    pub metrics: Vec<Metric>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resource {
    pub service: String,
    pub attributes: Attributes,
}

#[derive(Clone, Debug)]
pub struct Log {
    pub logged_at: i64,
    pub severity: Severity,
    pub body: String,
    pub trace_id: Option<TraceId>,
    pub span_id: Option<SpanId>,
    pub attributes: Attributes,
    pub source: &'static str,
}

#[derive(Clone, Debug)]
pub struct Span {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub name: String,
    pub kind: SpanKind,
    pub started_at: i64,
    pub duration_ns: i64,
    pub status: SpanStatus,
    pub attributes: Attributes,
    pub events: Vec<SpanEvent>,
}

#[derive(Clone, Debug)]
pub struct Metric {
    pub name: String,
    pub kind: MetricKind,
    pub unit: String,
    pub labels: Attributes,
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
    pub recorded_at: i64,
    pub value: f64,
    pub histogram: Option<Histogram>,
}
