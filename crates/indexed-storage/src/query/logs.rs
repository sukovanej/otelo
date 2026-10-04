use std::fmt;
use std::str::FromStr;

use jiff::Timestamp;
use otelo_query::{BuiltinField, Field, Signal, resolve_field};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PageCursor;
use crate::{Attributes, Severity, SpanId, TraceId};

pub const MAX_GROUPED_LOG_LINES: u64 = 50_000;

/// Log lines, newest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Logs {
    pub logs: Vec<LogLine>,
    /// The `after` of the request for the next page. Null when no more lines
    /// match.
    #[schema(value_type = Option<String>, required = true)]
    pub next: Option<PageCursor>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogGroupingField {
    Service,
    Level,
    Attribute(String),
    Resource(String),
}

impl FromStr for LogGroupingField {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match resolve_field(Signal::Logs, name) {
            Field::Builtin(BuiltinField::Service) => Ok(Self::Service),
            Field::Builtin(BuiltinField::Level) => Ok(Self::Level),
            Field::Attribute(key) => Ok(Self::Attribute(key)),
            Field::Resource(key) => Ok(Self::Resource(key)),
            Field::Builtin(builtin_field) => Err(format!(
                "logs group by service, level, an attribute, or resource.<key>, not {}",
                builtin_field.name()
            )),
        }
    }
}

impl fmt::Display for LogGroupingField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = match self {
            Self::Service => Field::Builtin(BuiltinField::Service),
            Self::Level => Field::Builtin(BuiltinField::Level),
            Self::Attribute(key) => Field::Attribute(key.clone()),
            Self::Resource(key) => Field::Resource(key.clone()),
        };
        field.fmt(formatter)
    }
}

/// The log lines a query keeps in a range, counted over the range and by
/// step, and grouped by the values of some fields, the most lines first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogCounts {
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub end_at: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    /// Every line the query keeps.
    pub count: u64,
    /// The lines of every step, oldest first.
    pub buckets: Vec<LogCountBucket>,
    pub groups: Vec<LogCountGroup>,
    /// More groups match than the limit let through, which kept the ones
    /// with the most lines.
    pub truncated: bool,
    /// The attributes the query compares that have no index.
    pub unindexed: Vec<String>,
}

/// The lines that share the values of the grouping fields.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogCountGroup {
    /// The values of the grouping fields, by the fields as a query writes
    /// them, such as `service`. A `level` is its name in lower case, such as
    /// `warn`. A field the lines lack is missing.
    pub values: Attributes,
    pub count: u64,
    /// The lines of the group in every step, oldest first.
    pub buckets: Vec<LogCountBucket>,
}

/// The lines of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogCountBucket {
    /// The start of the step.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    pub count: u64,
}
