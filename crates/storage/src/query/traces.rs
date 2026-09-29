use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::LogLine;
use crate::{Attributes, SpanEvent, SpanId, SpanKind, SpanStatus, TraceId};

/// Traces by their root span, newest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Traces {
    pub traces: Vec<TraceSummary>,
    /// More traces match than the limit let through.
    pub truncated: bool,
    /// The attributes the query compares that have no index.
    pub unindexed: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TraceSummary {
    #[schema(value_type = String)]
    pub trace_id: TraceId,
    /// The start of the root span.
    #[schema(value_type = String, format = DateTime)]
    pub started_at: Timestamp,
    pub service: String,
    /// The name of the root span.
    pub name: String,
    /// The OpenTelemetry span kind of the root span.
    #[schema(value_type = i32)]
    pub kind: SpanKind,
    pub duration_ns: i64,
    pub spans: u64,
    /// Whether a span of the trace failed.
    pub error: bool,
    /// The attributes of the root span, such as `http.route`.
    pub attributes: Attributes,
    /// The attributes of the resource that sent the root span.
    pub resource: Attributes,
}

/// Spans, newest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Spans {
    pub spans: Vec<TraceSpan>,
    /// More spans match than the limit let through.
    pub truncated: bool,
    /// The attributes the query compares that have no index.
    pub unindexed: Vec<String>,
}

/// One trace: its spans by start time, and the logs that carry its ID.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Trace {
    #[schema(value_type = String)]
    pub trace_id: TraceId,
    pub spans: Vec<TraceSpan>,
    /// Newest first.
    pub logs: Vec<LogLine>,
    /// The trace has more spans or logs than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TraceSpan {
    #[schema(value_type = String)]
    pub trace_id: TraceId,
    #[schema(value_type = String)]
    pub span_id: SpanId,
    #[schema(value_type = Option<String>, required = true)]
    pub parent_span_id: Option<SpanId>,
    pub service: String,
    pub name: String,
    /// The OpenTelemetry span kind.
    #[schema(value_type = i32)]
    pub kind: SpanKind,
    #[schema(value_type = String, format = DateTime)]
    pub started_at: Timestamp,
    pub duration_ns: i64,
    /// The OpenTelemetry status code: 2 when the span failed.
    #[schema(value_type = i32)]
    pub status: SpanStatus,
    pub attributes: Attributes,
    pub events: Vec<SpanEvent>,
    /// The attributes of the resource that sent the span.
    pub resource: Attributes,
}
