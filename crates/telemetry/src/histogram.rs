//! The buckets of a histogram point, and how the points of a time step merge.

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The buckets of one histogram point, as OpenTelemetry sends them. A point
/// keeps them as JSON in `points.histogram`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Histogram {
    /// The upper bounds of the buckets, ascending. The last bucket has no
    /// upper bound.
    pub bounds: Vec<f64>,
    /// How many values fell in each bucket: one more count than bounds.
    pub counts: Vec<u64>,
    pub count: u64,
    pub sum: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// The counts are since the series started, not since the point before.
    pub cumulative: bool,
}

impl Histogram {
    /// # Errors
    ///
    /// When the counts do not fit the bounds, or the bounds do not ascend.
    pub fn check(&self) -> anyhow::Result<()> {
        ensure!(
            self.counts.len() == self.bounds.len() + 1,
            "a histogram with {} bounds needs {} counts, not {}",
            self.bounds.len(),
            self.bounds.len() + 1,
            self.counts.len()
        );
        ensure!(
            self.bounds.windows(2).all(|pair| pair[0] < pair[1]),
            "the bounds of a histogram have to ascend"
        );
        Ok(())
    }

    /// What was added between `earlier` and this cumulative point. `None`
    /// when the series restarted or changed its bounds in between, so the
    /// point holds the whole increase.
    fn since(&self, earlier: &Self) -> Option<Self> {
        if self.bounds != earlier.bounds || self.count < earlier.count {
            return None;
        }
        let counts = self
            .counts
            .iter()
            .zip(&earlier.counts)
            .map(|(now, then)| now.checked_sub(*then))
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            bounds: self.bounds.clone(),
            counts,
            count: self.count - earlier.count,
            sum: self.sum.zip(earlier.sum).map(|(now, then)| now - then),
            min: None,
            max: None,
            cumulative: false,
        })
    }
}

/// The values a histogram series recorded in one time step: its points
/// merged, with the increases of cumulative points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Distribution {
    /// The upper bounds of the buckets. The last bucket has no upper bound.
    pub bounds: Vec<f64>,
    pub counts: Vec<u64>,
    pub count: u64,
    pub sum: Option<f64>,
    /// Estimates of the median and the 90th and 99th percentiles, by linear
    /// interpolation inside a bucket. `None` without values.
    pub p50: Option<f64>,
    pub p90: Option<f64>,
    pub p99: Option<f64>,
}

impl Distribution {
    fn new(histogram: Histogram) -> Self {
        Self {
            bounds: histogram.bounds,
            counts: histogram.counts,
            count: histogram.count,
            sum: histogram.sum,
            p50: None,
            p90: None,
            p99: None,
        }
    }

    /// Adds a delta. A point with other bounds replaces what came before, so
    /// the step keeps the newest bounds.
    fn add(&mut self, delta: Histogram) {
        if delta.bounds != self.bounds {
            *self = Self::new(delta);
            return;
        }
        for (count, more) in self.counts.iter_mut().zip(&delta.counts) {
            *count += more;
        }
        self.count += delta.count;
        self.sum = self.sum.zip(delta.sum).map(|(a, b)| a + b);
    }

    /// Fills the percentile estimates.
    fn estimate(&mut self) {
        self.p50 = self.quantile(0.5);
        self.p90 = self.quantile(0.9);
        self.p99 = self.quantile(0.99);
    }

    /// The value below which `q` of the values fall. A value in the first
    /// bucket counts from 0 when its bound is positive, and a value in the
    /// last bucket is its lower bound.
    #[expect(clippy::cast_precision_loss, reason = "an estimate")]
    fn quantile(&self, q: f64) -> Option<f64> {
        let total: u64 = self.counts.iter().sum();
        if total == 0 {
            return None;
        }
        let rank = q * total as f64;
        let mut below = 0.0;
        for (i, &count) in self.counts.iter().enumerate() {
            let count = count as f64;
            if count > 0.0 && below + count >= rank {
                let Some(&upper) = self.bounds.get(i) else {
                    return self.bounds.last().copied();
                };
                let lower = match i {
                    0 => upper.min(0.0),
                    _ => self.bounds[i - 1],
                };
                return Some((upper - lower).mul_add((rank - below) / count, lower));
            }
            below += count;
        }
        self.bounds.last().copied()
    }
}

/// Merges the histogram points of one series, in time order, into one
/// distribution per time step.
#[derive(Default)]
pub struct Merger {
    /// The last cumulative point, which the next one counts from.
    previous: Option<Histogram>,
    current: Option<(i64, Distribution)>,
    done: Vec<(i64, Distribution)>,
}

impl Merger {
    /// Adds the point at the start of time step `step`. The first cumulative
    /// point of a series only sets where the counting starts.
    pub fn push(&mut self, step: i64, point: Histogram) {
        let delta = if point.cumulative {
            let delta = self.previous.as_ref().map(|previous| {
                point.since(previous).unwrap_or_else(|| Histogram {
                    cumulative: false,
                    ..point.clone()
                })
            });
            self.previous = Some(point);
            match delta {
                Some(delta) => delta,
                None => return,
            }
        } else {
            point
        };
        match &mut self.current {
            Some((at, distribution)) if *at == step => distribution.add(delta),
            _ => {
                if let Some(done) = self.current.take() {
                    self.done.push(done);
                }
                self.current = Some((step, Distribution::new(delta)));
            }
        }
    }

    /// The distribution of each step that got values, with the estimates.
    #[must_use]
    pub fn finish(mut self) -> Vec<(i64, Distribution)> {
        self.done.extend(self.current.take());
        for (_, distribution) in &mut self.done {
            distribution.estimate();
        }
        self.done
    }
}
