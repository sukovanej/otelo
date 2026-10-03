use std::collections::BTreeMap;

use crate::{NumberPoint, Temporality};

const NANOS_PER_SECOND: f64 = 1e9;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Increase {
    pub counter_increase: f64,
    pub counter_increase_seconds: f64,
}

impl Increase {
    #[must_use]
    pub fn rate_per_second(self) -> Option<f64> {
        (self.counter_increase_seconds > 0.0)
            .then(|| self.counter_increase / self.counter_increase_seconds)
    }

    pub fn add_later_increase(&mut self, later: Self) {
        self.counter_increase += later.counter_increase;
        self.counter_increase_seconds += later.counter_increase_seconds;
    }
}

// Takes the points of one series, oldest first.
pub struct StepIncreases {
    temporality: Temporality,
    previous_point: Option<NumberPoint>,
    increases_by_step: BTreeMap<i64, Increase>,
}

impl StepIncreases {
    #[must_use]
    pub const fn new(temporality: Temporality) -> Self {
        Self {
            temporality,
            previous_point: None,
            increases_by_step: BTreeMap::new(),
        }
    }

    pub fn add_point(&mut self, step_start_at: i64, point: NumberPoint) {
        let previous_point = self.previous_point.replace(point);
        let amount = match (self.temporality, previous_point) {
            (Temporality::Cumulative, None) => return,
            (Temporality::Cumulative, Some(previous_point))
                if point.value >= previous_point.value =>
            {
                point.value - previous_point.value
            }
            // A cumulative value that fell is a counter that started again from zero.
            (Temporality::Delta, _) | (Temporality::Cumulative, Some(_)) => point.value,
        };
        #[expect(clippy::cast_precision_loss, reason = "a time between two points")]
        let elapsed_seconds = previous_point.map_or(0.0, |previous_point| {
            (point.recorded_at - previous_point.recorded_at) as f64 / NANOS_PER_SECOND
        });
        self.increases_by_step
            .entry(step_start_at)
            .or_default()
            .add_later_increase(Increase {
                counter_increase: amount,
                counter_increase_seconds: elapsed_seconds,
            });
    }

    #[must_use]
    pub fn into_increases_by_step(self) -> BTreeMap<i64, Increase> {
        self.increases_by_step
    }
}
