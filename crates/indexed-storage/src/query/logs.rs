use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Attributes, LogSource, Severity, SpanId, TraceId};

pub const MAX_GROUPED_LOG_LINES: u64 = 50_000;

/// Log lines, newest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Logs {
    pub logs: Vec<LogLine>,
    /// More lines match than the limit let through.
    pub truncated: bool,
    /// The attributes the query compares that have no index, so it read every
    /// line in the range.
    pub unindexed: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogLine {
    #[schema(value_type = String, format = DateTime)]
    pub logged_at: Timestamp,
    pub service: String,
    /// The OpenTelemetry severity number, from 1 (TRACE) to 24 (FATAL), or 0
    /// when the source set none.
    #[schema(value_type = i32)]
    pub severity: Severity,
    pub body: String,
    #[schema(value_type = Option<String>, required = true)]
    pub trace_id: Option<TraceId>,
    #[schema(value_type = Option<String>, required = true)]
    pub span_id: Option<SpanId>,
    pub attributes: Attributes,
    /// The attributes of the resource that sent the line.
    pub resource: Attributes,
    /// `otlp`, or the service log source that read the line.
    pub source: LogSource,
}

/// Log lines grouped by message template, the largest group first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogGroups {
    pub groups: Vec<LogGroup>,
    /// More groups exist than the limit let through.
    pub truncated: bool,
    /// How many lines the groups count.
    pub scanned: u64,
    /// The range has more lines than the grouping reads, so the groups count
    /// only the newest ones.
    pub partial: bool,
    /// The attributes the query compares that have no index.
    pub unindexed: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogGroup {
    /// The body with numbers, UUIDs, hex IDs, and quoted strings replaced by
    /// placeholders.
    pub template: String,
    pub count: u64,
    /// The highest severity number in the group.
    #[schema(value_type = i32)]
    pub severity: Severity,
    pub services: Vec<String>,
    #[schema(value_type = String, format = DateTime)]
    pub first_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub last_at: Timestamp,
    /// Up to three different bodies, newest first.
    pub samples: Vec<String>,
}
