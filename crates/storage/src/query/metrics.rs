use jiff::Timestamp;
use otelo_query::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Attributes, Distribution, TimeRange};

pub const MAX_BUCKETS: i64 = 10_000;

const SECOND: i64 = 1_000_000_000;

const ROUND_STEPS: [i64; 15] = [
    SECOND,
    5 * SECOND,
    10 * SECOND,
    15 * SECOND,
    30 * SECOND,
    60 * SECOND,
    5 * 60 * SECOND,
    10 * 60 * SECOND,
    15 * 60 * SECOND,
    30 * 60 * SECOND,
    3600 * SECOND,
    3 * 3600 * SECOND,
    6 * 3600 * SECOND,
    12 * 3600 * SECOND,
    86_400 * SECOND,
];

#[must_use]
pub fn default_step(range: TimeRange) -> i64 {
    step_for(range, 120)
}

#[must_use]
pub fn step_for(range: TimeRange, buckets: i64) -> i64 {
    ROUND_STEPS
        .into_iter()
        .find(|step| range.length() / step <= buckets)
        .unwrap_or(ROUND_STEPS[ROUND_STEPS.len() - 1])
}

/// The series in a range, by name.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricList {
    pub series: Vec<SeriesInfo>,
    /// More series exist than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SeriesInfo {
    pub name: String,
    /// `gauge`, `sum`, or `histogram`.
    pub kind: String,
    pub unit: String,
    pub service: String,
    pub labels: Attributes,
    /// The attributes of the resource that sends the series.
    pub resource: Attributes,
}

#[derive(Clone, Debug)]
pub struct MetricFilter {
    pub name: String,
    pub query: Query,
    pub step_ns: i64,
}

/// The series of one metric, each in buckets of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricSeries {
    pub name: String,
    pub step_ns: i64,
    pub series: Vec<Series>,
    /// More series match than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Series {
    pub service: String,
    pub kind: String,
    pub unit: String,
    pub labels: Attributes,
    pub resource: Attributes,
    /// The buckets that have points, oldest first.
    pub buckets: Vec<Bucket>,
}

/// The points of one series in one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Bucket {
    /// The start of the bucket.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    /// The value of the newest point.
    pub last: f64,
    /// The values a histogram recorded in the step: the counts of its
    /// buckets and percentile estimates. `None` for a gauge or a sum, and for
    /// the first step of a cumulative histogram, which only sets where the
    /// counting starts.
    #[schema(required = true)]
    pub histogram: Option<Distribution>,
}
