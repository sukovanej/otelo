mod attributes;
mod error;
mod grouping;
mod histogram;
mod increase;
mod indexes;
mod indexing;
mod meters;
mod metric;
mod otel;
pub mod query;
mod range;
mod storage;
mod summary;

pub use attributes::{AttributeValue, Attributes, SpanEvent};
pub use error::{Error, Result};
pub use grouping::{SummarizedSeries, group_series};
pub use histogram::{
    Buckets, Distribution, ExplicitBuckets, ExponentialBuckets, Histogram, IndexedCounts,
    Percentiles, StepHistograms,
};
pub use increase::{Increase, StepIncreases};
pub use indexes::{IndexedAttribute, IndexedSignal};
pub use indexing::{Indexing, SignalIndexing};
pub use meters::{
    DURATION_BUCKET_BOUNDS_SECONDS, DurationCounts, FrameCounts, PipelineMeters, PipelineReading,
    RecordCounts, RequestCounts, SignalReading,
};
pub use metric::{HistogramPoint, Metric, MetricKind, NumberPoint, Points, Temporality};
pub use otel::{Severity, SpanId, SpanKind, SpanStatus, TraceContext, TraceId};
pub use range::TimeRange;
pub use storage::{RangeQueries, Storage, StorageSize};
pub use summary::{Change, Level, SeriesPoint, SeriesSteps, StepSummary};

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
    pub severity_number: Severity,
    pub body: String,
    pub trace_context: TraceContext,
    pub attributes: Attributes,
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
    pub status_code: SpanStatus,
    pub attributes: Attributes,
    pub events: Vec<SpanEvent>,
}
