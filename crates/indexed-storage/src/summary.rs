use std::collections::BTreeMap;

use crate::{
    Histogram, HistogramPoint, Increase, MetricKind, NumberPoint, StepHistograms, StepIncreases,
};

#[derive(Clone, Debug, PartialEq)]
pub enum SeriesPoint {
    Number(NumberPoint),
    Histogram(HistogramPoint),
}

impl SeriesPoint {
    #[must_use]
    pub const fn recorded_at(&self) -> i64 {
        match self {
            Self::Number(point) => point.recorded_at,
            Self::Histogram(point) => point.recorded_at,
        }
    }

    // The store keeps the sum of a histogram point as the value of its row.
    #[must_use]
    pub fn stored_value(&self) -> f64 {
        match self {
            Self::Number(point) => point.value,
            Self::Histogram(point) => point.histogram.sum.unwrap_or(0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub point_count: u64,
    pub min_value: f64,
    pub max_value: f64,
    pub value_sum: f64,
    pub last_value: f64,
}

impl Level {
    #[must_use]
    pub const fn from_value(value: f64) -> Self {
        Self {
            point_count: 1,
            min_value: value,
            max_value: value,
            value_sum: value,
            last_value: value,
        }
    }

    pub const fn add_value(&mut self, value: f64) {
        self.point_count += 1;
        self.min_value = self.min_value.min(value);
        self.max_value = self.max_value.max(value);
        self.value_sum += value;
        self.last_value = value;
    }

    pub const fn add_later_level(&mut self, later: Self) {
        self.point_count += later.point_count;
        self.min_value = self.min_value.min(later.min_value);
        self.max_value = self.max_value.max(later.max_value);
        self.value_sum += later.value_sum;
        self.last_value = later.last_value;
    }

    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "a count below 2^53")]
    pub fn average(self) -> f64 {
        self.value_sum / self.point_count as f64
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Nothing,
    Increase(Increase),
    Distribution(Box<Histogram>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepSummary {
    pub level: Level,
    pub change: Change,
}

impl StepSummary {
    pub fn add_later_summary(&mut self, later: Self) {
        self.level.add_later_level(later.level);
        match (&mut self.change, later.change) {
            (_, Change::Nothing) => {}
            (Change::Increase(increase), Change::Increase(later_increase)) => {
                increase.add_later_increase(later_increase);
            }
            (Change::Distribution(histogram), Change::Distribution(later_histogram)) => {
                histogram.add_increase(*later_histogram);
            }
            (change, later_change) => *change = later_change,
        }
    }
}

enum ChangesByStep {
    Nothing,
    Increases(StepIncreases),
    Distributions(Box<StepHistograms>),
}

// Takes the points of one series, oldest first.
pub struct SeriesSteps {
    levels_by_step: BTreeMap<i64, Level>,
    changes_by_step: ChangesByStep,
}

impl SeriesSteps {
    #[must_use]
    pub fn new(kind: MetricKind) -> Self {
        Self {
            levels_by_step: BTreeMap::new(),
            changes_by_step: match kind {
                MetricKind::Gauge | MetricKind::UpDown => ChangesByStep::Nothing,
                MetricKind::Counter(temporality) => {
                    ChangesByStep::Increases(StepIncreases::new(temporality))
                }
                MetricKind::Histogram(temporality) => {
                    ChangesByStep::Distributions(Box::new(StepHistograms::new(temporality)))
                }
            },
        }
    }

    pub fn add_point(&mut self, step_start_at: i64, point: SeriesPoint) {
        let value = point.stored_value();
        self.add_point_before_range(step_start_at, point);
        self.levels_by_step
            .entry(step_start_at)
            .and_modify(|level| level.add_value(value))
            .or_insert_with(|| Level::from_value(value));
    }

    // A counter and a cumulative histogram count from the point before, so a point before the
    // range only says where the counting starts.
    pub fn add_point_before_range(&mut self, step_start_at: i64, point: SeriesPoint) {
        match (&mut self.changes_by_step, point) {
            (ChangesByStep::Increases(increases), SeriesPoint::Number(point)) => {
                increases.add_point(step_start_at, point);
            }
            (ChangesByStep::Distributions(histograms), SeriesPoint::Histogram(point)) => {
                histograms.add_point(step_start_at, point.histogram);
            }
            (ChangesByStep::Nothing, _)
            | (ChangesByStep::Increases(_), SeriesPoint::Histogram(_))
            | (ChangesByStep::Distributions(_), SeriesPoint::Number(_)) => {}
        }
    }

    #[must_use]
    pub fn has_no_point_in_range(&self) -> bool {
        self.levels_by_step.is_empty()
    }

    #[must_use]
    pub fn into_summaries_by_step(self) -> BTreeMap<i64, StepSummary> {
        let mut summaries_by_step: BTreeMap<i64, StepSummary> = self
            .levels_by_step
            .into_iter()
            .map(|(step_start_at, level)| {
                let change = Change::Nothing;
                (step_start_at, StepSummary { level, change })
            })
            .collect();
        match self.changes_by_step {
            ChangesByStep::Nothing => {}
            ChangesByStep::Increases(increases) => {
                for (step_start_at, increase) in increases.into_increases_by_step() {
                    if let Some(summary) = summaries_by_step.get_mut(&step_start_at) {
                        summary.change = Change::Increase(increase);
                    }
                }
            }
            ChangesByStep::Distributions(histograms) => {
                for (step_start_at, histogram) in histograms.into_histograms_by_step() {
                    if let Some(summary) = summaries_by_step.get_mut(&step_start_at) {
                        summary.change = Change::Distribution(Box::new(histogram));
                    }
                }
            }
        }
        summaries_by_step
    }
}
