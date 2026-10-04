use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use jiff::Timestamp;
use otelo_query::{BuiltinField, Field, Signal, resolve_field};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::RankOrder;
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

/// The number span groups rank by: the total `time` of their spans, their
/// `count`, their `errors`, their `error_rate`, or a percentile of their
/// durations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpanGroupRank {
    #[default]
    Time,
    Count,
    Errors,
    ErrorRate,
    P50,
    P95,
    P99,
}

impl SpanGroupRank {
    pub const ALL: [Self; 7] = [
        Self::Time,
        Self::Count,
        Self::Errors,
        Self::ErrorRate,
        Self::P50,
        Self::P95,
        Self::P99,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Count => "count",
            Self::Errors => "errors",
            Self::ErrorRate => "error_rate",
            Self::P50 => "p50",
            Self::P95 => "p95",
            Self::P99 => "p99",
        }
    }

    fn compare_lowest_first(self, a: &SpanStats, b: &SpanStats) -> Ordering {
        let percentile =
            |stats: &SpanStats, pick: fn(&Latency) -> i64| stats.latency.as_ref().map(pick);
        match self {
            Self::Time => a.total_ns.cmp(&b.total_ns),
            Self::Count => a.count.cmp(&b.count),
            Self::Errors => a.errors.cmp(&b.errors),
            Self::ErrorRate => (u128::from(a.errors) * u128::from(b.count))
                .cmp(&(u128::from(b.errors) * u128::from(a.count))),
            Self::P50 => {
                percentile(a, |latency| latency.p50).cmp(&percentile(b, |latency| latency.p50))
            }
            Self::P95 => {
                percentile(a, |latency| latency.p95).cmp(&percentile(b, |latency| latency.p95))
            }
            Self::P99 => {
                percentile(a, |latency| latency.p99).cmp(&percentile(b, |latency| latency.p99))
            }
        }
    }
}

impl FromStr for SpanGroupRank {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|rank| rank.name() == name)
            .ok_or_else(|| {
                format!("{name:?} is not a rank: time, count, errors, error_rate, p50, p95, or p99")
            })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpanGroupRanking {
    pub rank: SpanGroupRank,
    pub order: RankOrder,
}

impl SpanGroupRanking {
    #[must_use]
    pub fn compare(self, a: &SpanStats, b: &SpanStats) -> Ordering {
        self.order
            .orient_ordering(self.rank.compare_lowest_first(a, b))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupBuckets {
    Omitted,
    Counted,
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
    /// The spans of the group in every step, oldest first, when
    /// `group_buckets` asked for them.
    #[schema(required = true)]
    pub buckets: Option<Vec<SpanBucket>>,
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
