use std::collections::BTreeMap;

use crate::{NumberPoint, Temporality};

const NANOS_PER_SECOND: f64 = 1e9;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Increase {
    pub amount: f64,
    pub seconds: f64,
}

impl Increase {
    #[must_use]
    pub fn rate_per_second(self) -> Option<f64> {
        (self.seconds > 0.0).then(|| self.amount / self.seconds)
    }

    pub fn add_later_increase(&mut self, later: Self) {
        self.amount += later.amount;
        self.seconds += later.seconds;
    }
}

// Takes the points of one series, oldest first.
pub struct StepIncreases {
    temporality: Temporality,
    previous_point: Option<NumberPoint>,
    by_step: BTreeMap<i64, Increase>,
}

impl StepIncreases {
    #[must_use]
    pub const fn new(temporality: Temporality) -> Self {
        Self {
            temporality,
            previous_point: None,
            by_step: BTreeMap::new(),
        }
    }

    pub fn add_point(&mut self, step: i64, point: NumberPoint) {
        let previous = self.previous_point.replace(point);
        let amount = match (self.temporality, previous) {
            (Temporality::Cumulative, None) => return,
            (Temporality::Cumulative, Some(previous)) if point.value >= previous.value => {
                point.value - previous.value
            }
            // A cumulative value that fell is a counter that started again from zero.
            (Temporality::Delta, _) | (Temporality::Cumulative, Some(_)) => point.value,
        };
        #[expect(clippy::cast_precision_loss, reason = "a time between two points")]
        let seconds = previous.map_or(0.0, |previous| {
            (point.recorded_at - previous.recorded_at) as f64 / NANOS_PER_SECOND
        });
        self.by_step
            .entry(step)
            .or_default()
            .add_later_increase(Increase { amount, seconds });
    }

    #[must_use]
    pub fn into_increases_by_step(self) -> BTreeMap<i64, Increase> {
        self.by_step
    }
}
