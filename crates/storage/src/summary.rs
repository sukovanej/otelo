use std::collections::BTreeMap;

use crate::{Histogram, Increase, MetricKind, NumberPoint, StepHistograms, StepIncreases};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub last: f64,
}

impl Level {
    #[must_use]
    pub const fn of_point(value: f64) -> Self {
        Self {
            count: 1,
            min: value,
            max: value,
            sum: value,
            last: value,
        }
    }

    pub const fn add_point(&mut self, value: f64) {
        self.count += 1;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.last = value;
    }

    pub const fn add_later_level(&mut self, later: Self) {
        self.count += later.count;
        self.min = self.min.min(later.min);
        self.max = self.max.max(later.max);
        self.sum += later.sum;
        self.last = later.last;
    }

    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "a count below 2^53")]
    pub fn average(self) -> f64 {
        self.sum / self.count as f64
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
            (Change::Increase(sum), Change::Increase(more)) => sum.add_later_increase(more),
            (Change::Distribution(sum), Change::Distribution(more)) => sum.add_increase(*more),
            (change, later) => *change = later,
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
    levels: BTreeMap<i64, Level>,
    changes: ChangesByStep,
}

impl SeriesSteps {
    #[must_use]
    pub fn of_kind(kind: MetricKind) -> Self {
        Self {
            levels: BTreeMap::new(),
            changes: match kind {
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

    pub fn add_point(&mut self, step: i64, point: NumberPoint, histogram: Option<Histogram>) {
        self.add_point_before_range(step, point, histogram);
        self.levels
            .entry(step)
            .and_modify(|level| level.add_point(point.value))
            .or_insert_with(|| Level::of_point(point.value));
    }

    // A counter and a cumulative histogram count from the point before, so a point before the
    // range only says where the counting starts.
    pub fn add_point_before_range(
        &mut self,
        step: i64,
        point: NumberPoint,
        histogram: Option<Histogram>,
    ) {
        match &mut self.changes {
            ChangesByStep::Nothing => {}
            ChangesByStep::Increases(increases) => increases.add_point(step, point),
            ChangesByStep::Distributions(histograms) => {
                if let Some(histogram) = histogram {
                    histograms.add_point(step, histogram);
                }
            }
        }
    }

    #[must_use]
    pub fn has_no_point_in_range(&self) -> bool {
        self.levels.is_empty()
    }

    #[must_use]
    pub fn into_summaries_by_step(self) -> BTreeMap<i64, StepSummary> {
        let mut steps: BTreeMap<i64, StepSummary> = self
            .levels
            .into_iter()
            .map(|(step, level)| {
                let change = Change::Nothing;
                (step, StepSummary { level, change })
            })
            .collect();
        match self.changes {
            ChangesByStep::Nothing => {}
            ChangesByStep::Increases(increases) => {
                for (step, increase) in increases.into_increases_by_step() {
                    if let Some(summary) = steps.get_mut(&step) {
                        summary.change = Change::Increase(increase);
                    }
                }
            }
            ChangesByStep::Distributions(histograms) => {
                for (step, merged) in histograms.into_histograms_by_step() {
                    if let Some(summary) = steps.get_mut(&step) {
                        summary.change = Change::Distribution(Box::new(merged));
                    }
                }
            }
        }
        steps
    }
}
