use std::fmt;
use std::str::FromStr;

use jiff::Timestamp;
use otelo_query::{BuiltinField, Field, Signal, resolve_field};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Attributes;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanGroupingField {
    Service,
    Name,
    Attribute(String),
    Resource(String),
}

impl FromStr for SpanGroupingField {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match resolve_field(Signal::Spans, name) {
            Field::Builtin(BuiltinField::Service) => Ok(Self::Service),
            Field::Builtin(BuiltinField::Name) => Ok(Self::Name),
            Field::Attribute(key) => Ok(Self::Attribute(key)),
            Field::Resource(key) => Ok(Self::Resource(key)),
            Field::Builtin(builtin_field) => Err(format!(
                "spans group by service, name, an attribute, or resource.<key>, not {}",
                builtin_field.name()
            )),
        }
    }
}

impl fmt::Display for SpanGroupingField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = match self {
            Self::Service => Field::Builtin(BuiltinField::Service),
            Self::Name => Field::Builtin(BuiltinField::Name),
            Self::Attribute(key) => Field::Attribute(key.clone()),
            Self::Resource(key) => Field::Resource(key.clone()),
        };
        field.fmt(formatter)
    }
}

/// The spans a query keeps in a range, grouped by the values of some fields,
/// the most time first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SpanGroups {
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub end_at: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    /// Every span the query keeps.
    pub spans: SpanStats,
    /// The spans of every step, oldest first.
    pub buckets: Vec<SpanBucket>,
    pub groups: Vec<SpanGroup>,
    /// More groups match than the limit let through, which kept the ones
    /// with the most time.
    pub truncated: bool,
    /// The attributes the query compares that have no index.
    pub unindexed: Vec<String>,
}

/// The spans that share the values of the grouping fields.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SpanGroup {
    /// The values of the grouping fields, by the fields as a query writes
    /// them, such as `http.route`. A field the spans lack is missing.
    pub values: Attributes,
    /// The span name of its newest span.
    pub name: String,
    /// The attributes of its newest span.
    pub attributes: Attributes,
    pub spans: SpanStats,
}

/// The spans of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SpanBucket {
    /// The start of the step.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    pub spans: SpanStats,
}

/// The count, the failures, and the durations of some spans.
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct SpanStats {
    pub count: u64,
    /// The failed ones.
    pub errors: u64,
    /// Their durations added up.
    pub total_ns: i64,
    /// Estimates of the percentiles of their durations, off by 1% at most.
    /// `None` without spans.
    #[schema(required = true)]
    pub latency: Option<Latency>,
}

/// Percentiles of durations, in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Latency {
    pub p50: i64,
    pub p95: i64,
    pub p99: i64,
}
