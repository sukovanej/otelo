use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Attributes, Histogram};

#[derive(Clone, Debug)]
pub struct Metric {
    pub name: String,
    pub unit: String,
    pub labels: Attributes,
    pub points: Points,
}

#[derive(Clone, Debug)]
pub enum Points {
    Gauge(Vec<NumberPoint>),
    UpDown(Vec<NumberPoint>),
    Counter(Temporality, Vec<NumberPoint>),
    Histogram(Temporality, Vec<HistogramPoint>),
}

impl Points {
    #[must_use]
    pub const fn kind(&self) -> MetricKind {
        match self {
            Self::Gauge(_) => MetricKind::Gauge,
            Self::UpDown(_) => MetricKind::UpDown,
            Self::Counter(temporality, _) => MetricKind::Counter(*temporality),
            Self::Histogram(temporality, _) => MetricKind::Histogram(*temporality),
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        match self {
            Self::Gauge(points) | Self::UpDown(points) | Self::Counter(_, points) => points.len(),
            Self::Histogram(_, points) => points.len(),
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberPoint {
    pub recorded_at: i64,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistogramPoint {
    pub recorded_at: i64,
    pub histogram: Histogram,
}

/// How the points of a series combine over time.
///
/// A `gauge` is a value at an instant, such as a CPU share. An `updown` is a
/// level that goes up and down, such as the memory in use, and the series of
/// one metric add up. A `counter` is a total that only grows, such as the
/// bytes sent, and a chart shows its rate. A `histogram` is the distribution
/// of many values, such as request durations. A `counter` and a `histogram`
/// have a temporality.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(tag = "kind", content = "temporality", rename_all = "lowercase")]
pub enum MetricKind {
    Gauge,
    UpDown,
    Counter(Temporality),
    Histogram(Temporality),
}

impl MetricKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gauge => "gauge",
            Self::UpDown => "updown",
            Self::Counter(_) => "counter",
            Self::Histogram(_) => "histogram",
        }
    }

    #[must_use]
    pub const fn temporality(self) -> Option<Temporality> {
        match self {
            Self::Gauge | Self::UpDown => None,
            Self::Counter(temporality) | Self::Histogram(temporality) => Some(temporality),
        }
    }

    #[must_use]
    pub fn from_name_and_temporality(name: &str, temporality: Option<&str>) -> Option<Self> {
        let temporality = temporality.map(Temporality::from_name);
        match (name, temporality) {
            ("gauge", None) => Some(Self::Gauge),
            ("updown", None) => Some(Self::UpDown),
            ("counter", Some(Some(temporality))) => Some(Self::Counter(temporality)),
            ("histogram", Some(Some(temporality))) => Some(Self::Histogram(temporality)),
            _ => None,
        }
    }
}

/// What a point of a series counts: `cumulative` since the series started, or
/// `delta` since the point before.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Temporality {
    Cumulative,
    Delta,
}

impl Temporality {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cumulative => "cumulative",
            Self::Delta => "delta",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "cumulative" => Some(Self::Cumulative),
            "delta" => Some(Self::Delta),
            _ => None,
        }
    }
}
