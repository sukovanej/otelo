use std::str::FromStr;

use jiff::Timestamp;
use otelo_query::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    Attributes, Change, Distribution, MetricKind, MetricRetention, StepSummary, TimeRange,
};

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
    #[serde(flatten)]
    pub kind: MetricKind,
    pub unit: String,
    pub service: String,
    pub labels: Attributes,
    /// The attributes of the resource that sends the series.
    pub resource: Attributes,
}

// A longer range would read more raw points than a chart has pixels.
const MAX_RAW_RANGE_NS: i64 = 6 * 3600 * SECOND;
const MAX_MINUTE_RANGE_NS: i64 = 14 * 86_400 * SECOND;

/// Which points a metric query reads.
///
/// `raw` is the points as they arrived. `1m` and `1h` are their summaries
/// by the minute and by the hour, which outlive them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum Resolution {
    #[serde(rename = "raw")]
    Raw,
    #[serde(rename = "1m")]
    Minute,
    #[serde(rename = "1h")]
    Hour,
}

impl Resolution {
    #[must_use]
    pub const fn finest_kept_for(range: TimeRange, retention: MetricRetention) -> Self {
        if range.length() <= MAX_RAW_RANGE_NS && range.start_at() >= retention.oldest_raw_at {
            Self::Raw
        } else if range.length() <= MAX_MINUTE_RANGE_NS
            && range.start_at() >= retention.oldest_minute_at
        {
            Self::Minute
        } else {
            Self::Hour
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Minute => "1m",
            Self::Hour => "1h",
        }
    }
}

impl FromStr for Resolution {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        [Self::Raw, Self::Minute, Self::Hour]
            .into_iter()
            .find(|resolution| resolution.name() == name)
            .ok_or_else(|| format!("{name:?} is not a resolution: raw, 1m, or 1h"))
    }
}

#[derive(Clone, Debug)]
pub struct MetricFilter {
    pub name: String,
    pub query: Query,
    pub step_ns: i64,
    pub resolution: Resolution,
}

/// The series of one metric, each in buckets of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricSeries {
    pub name: String,
    /// The length of a bucket. A query of the summaries by the minute or by
    /// the hour rounds the step up to whole minutes or hours.
    pub step_ns: i64,
    pub resolution: Resolution,
    pub series: Vec<Series>,
    /// More series match than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Series {
    pub service: String,
    #[serde(flatten)]
    pub kind: MetricKind,
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
    pub change: BucketChange,
}

/// What the points of a step say beyond their values.
///
/// `none` for a gauge and an updown, and for a step with only the first
/// point of a cumulative series, which has nothing to count from. `rate` is
/// how much a counter grew per second: its increase between neighbouring
/// points, over the time between them. A value that goes down is a restart,
/// and the increase counts from zero. `distribution` is the merged buckets
/// of a histogram with percentile estimates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BucketChange {
    None,
    Rate { per_second: f64 },
    Distribution(Distribution),
}

impl Bucket {
    #[must_use]
    pub fn of_step(start_at: Timestamp, summary: StepSummary) -> Self {
        let change = match summary.change {
            Change::Nothing => BucketChange::None,
            Change::Increase(increase) => increase
                .rate_per_second()
                .map_or(BucketChange::None, |per_second| BucketChange::Rate {
                    per_second,
                }),
            Change::Distribution(histogram) => {
                BucketChange::Distribution(Distribution::from(*histogram))
            }
        };
        Self {
            start_at,
            count: summary.level.count,
            min: summary.level.min,
            max: summary.level.max,
            avg: summary.level.average(),
            last: summary.level.last,
            change,
        }
    }
}
