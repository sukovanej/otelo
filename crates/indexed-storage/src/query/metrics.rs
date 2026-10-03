use std::fmt;
use std::num::NonZeroUsize;
use std::str::FromStr;

use jiff::Timestamp;
use otelo_query::{BuiltinField, Field, Query, Signal, resolve_field};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Attributes, Distribution, MetricKind, TimeRange};

pub const MAX_BUCKETS_IN_RANGE: i64 = 10_000;

const NANOS_PER_SECOND: i64 = 1_000_000_000;

const ROUND_STEPS_NS: [i64; 15] = [
    NANOS_PER_SECOND,
    5 * NANOS_PER_SECOND,
    10 * NANOS_PER_SECOND,
    15 * NANOS_PER_SECOND,
    30 * NANOS_PER_SECOND,
    60 * NANOS_PER_SECOND,
    5 * 60 * NANOS_PER_SECOND,
    10 * 60 * NANOS_PER_SECOND,
    15 * 60 * NANOS_PER_SECOND,
    30 * 60 * NANOS_PER_SECOND,
    3600 * NANOS_PER_SECOND,
    3 * 3600 * NANOS_PER_SECOND,
    6 * 3600 * NANOS_PER_SECOND,
    12 * 3600 * NANOS_PER_SECOND,
    86_400 * NANOS_PER_SECOND,
];

#[must_use]
pub fn choose_default_step_ns(range: TimeRange) -> i64 {
    choose_round_step_ns(range, 120)
}

#[must_use]
pub fn choose_round_step_ns(range: TimeRange, max_buckets: i64) -> i64 {
    ROUND_STEPS_NS
        .into_iter()
        .find(|step_ns| range.length_ns() / step_ns <= max_buckets)
        .unwrap_or(ROUND_STEPS_NS[ROUND_STEPS_NS.len() - 1])
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
    pub attributes: Attributes,
    /// The attributes of the resource that sends the series.
    pub resource: Attributes,
}

// A longer range would read more raw points than a chart has pixels.
const MAX_RAW_RANGE_NS: i64 = 6 * 3600 * NANOS_PER_SECOND;
const MAX_MINUTE_RANGE_NS: i64 = 14 * 86_400 * NANOS_PER_SECOND;

/// Which points a metric query reads.
///
/// `raw` is the points as they arrived. `1m` and `1h` are their summaries
/// by the minute and by the hour, which a long range reads in fewer rows.
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
    pub const fn choose_for_range_length(range: TimeRange) -> Self {
        if range.length_ns() <= MAX_RAW_RANGE_NS {
            Self::Raw
        } else if range.length_ns() <= MAX_MINUTE_RANGE_NS {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupingField {
    Service,
    Attribute(String),
    Resource(String),
}

impl FromStr for GroupingField {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match resolve_field(Signal::Metrics, name) {
            Field::Builtin(BuiltinField::Service) => Ok(Self::Service),
            Field::Attribute(key) => Ok(Self::Attribute(key)),
            Field::Resource(key) => Ok(Self::Resource(key)),
            Field::Builtin(builtin_field) => Err(format!(
                "the series of a metric share its {}; group them by an attribute, service, or \
                 resource.<key>",
                builtin_field.name()
            )),
        }
    }
}

impl fmt::Display for GroupingField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = match self {
            Self::Service => Field::Builtin(BuiltinField::Service),
            Self::Attribute(key) => Field::Attribute(key.clone()),
            Self::Resource(key) => Field::Resource(key.clone()),
        };
        field.fmt(formatter)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Grouping {
    pub by: Vec<GroupingField>,
    pub top: Option<NonZeroUsize>,
}

#[derive(Clone, Debug)]
pub struct MetricFilter {
    pub name: String,
    pub query: Query,
    pub step_ns: i64,
    pub resolution: Resolution,
    pub grouping: Grouping,
}

/// The series of one metric, or the groups of them, each in buckets of one
/// step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct MetricSeries {
    pub name: String,
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub end_at: Timestamp,
    /// The length of a bucket. A query of the summaries by the minute or by
    /// the hour rounds the step up to whole minutes or hours.
    pub step_ns: i64,
    pub resolution: Resolution,
    /// The highest value over the range first: the average of a gauge and an
    /// updown, the rate of a counter, and the sum of the values of a
    /// histogram. The group `other` comes last.
    pub groups: Vec<SeriesGroup>,
    /// More groups match than the limit let through, or the metric has more
    /// series than a query reads.
    pub truncated: bool,
}

/// One series, or the series of a group combined in each step.
///
/// A gauge takes their average, an updown and a counter add up, and a
/// histogram merges its buckets. The minimum and the maximum of series that
/// add up are the sums of theirs, since their points do not line up in time.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SeriesGroup {
    pub key: GroupKey,
    #[serde(flatten)]
    pub kind: MetricKind,
    pub unit: String,
    /// The buckets that have points, oldest first.
    pub buckets: Vec<Bucket>,
}

/// What a group holds.
///
/// `series` is one series, when the query groups by nothing. `values` is
/// the series that have these values of the names of `by`, and a name that
/// the series lack is missing. `other` is the groups past `top`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum GroupKey {
    Series {
        service: String,
        attributes: Attributes,
        /// The attributes of the resource that sends the series.
        resource: Attributes,
    },
    Values {
        values: Attributes,
        series_count: usize,
    },
    Other {
        group_count: usize,
        series_count: usize,
    },
}

/// The points of one series or group in one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Bucket {
    /// The start of the bucket.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    /// The value of the newest point. In a group, the newest points of its
    /// series combined.
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
