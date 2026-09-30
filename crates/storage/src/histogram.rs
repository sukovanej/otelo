use std::collections::BTreeMap;

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Temporality;

const OTLP_EXPONENTIAL_SCALES: std::ops::RangeInclusive<i32> = -10..=20;

const MAX_DISTRIBUTION_BUCKETS: usize = 64;

// Two points far apart in their indexes would otherwise span buckets without end.
const MAX_SPANNED_BUCKETS: i64 = 4096;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Histogram {
    pub count: u64,
    pub sum: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    #[serde(flatten)]
    pub buckets: Buckets,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Buckets {
    Explicit(ExplicitBuckets),
    Exponential(ExponentialBuckets),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExplicitBuckets {
    pub bounds: Vec<f64>,
    pub counts: Vec<u64>,
}

// Bucket `index` of a side holds the values from `base^index` to `base^(index + 1)`, with
// `base = 2^(2^-scale)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExponentialBuckets {
    pub scale: i32,
    pub zero_count: u64,
    pub positive: IndexedCounts,
    pub negative: IndexedCounts,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedCounts {
    pub offset: i32,
    pub counts: Vec<u64>,
}

impl Histogram {
    pub fn check_buckets(&self) -> anyhow::Result<()> {
        match &self.buckets {
            Buckets::Explicit(explicit) => explicit.check_counts_against_bounds(),
            Buckets::Exponential(exponential) => exponential.check_scale_and_size(),
        }
    }

    fn increase_since(&self, earlier: &Self) -> Option<Self> {
        if self.count < earlier.count {
            return None;
        }
        let buckets = match (&self.buckets, &earlier.buckets) {
            (Buckets::Explicit(now), Buckets::Explicit(then)) => {
                Buckets::Explicit(now.subtract_counts_of(then)?)
            }
            (Buckets::Exponential(now), Buckets::Exponential(then)) => {
                Buckets::Exponential(now.subtract_counts_of(then)?)
            }
            _ => return None,
        };
        Some(Self {
            count: self.count - earlier.count,
            sum: self.sum.zip(earlier.sum).map(|(now, then)| now - then),
            min: None,
            max: None,
            buckets,
        })
    }

    pub fn add_increase(&mut self, increase: Self) {
        let buckets = match (&self.buckets, &increase.buckets) {
            (Buckets::Explicit(sum), Buckets::Explicit(more)) => {
                sum.add_counts_of(more).map(Buckets::Explicit)
            }
            (Buckets::Exponential(sum), Buckets::Exponential(more)) => {
                sum.add_counts_of(more).map(Buckets::Exponential)
            }
            _ => None,
        };
        let Some(buckets) = buckets else {
            *self = increase;
            return;
        };
        self.buckets = buckets;
        self.count += increase.count;
        self.sum = self.sum.zip(increase.sum).map(|(a, b)| a + b);
        self.min = self.min.zip(increase.min).map(|(a, b)| a.min(b));
        self.max = self.max.zip(increase.max).map(|(a, b)| a.max(b));
    }
}

impl ExplicitBuckets {
    fn check_counts_against_bounds(&self) -> anyhow::Result<()> {
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

    fn subtract_counts_of(&self, earlier: &Self) -> Option<Self> {
        if self.bounds != earlier.bounds {
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
        })
    }

    fn add_counts_of(&self, more: &Self) -> Option<Self> {
        if self.bounds != more.bounds {
            return None;
        }
        Some(Self {
            bounds: self.bounds.clone(),
            counts: self
                .counts
                .iter()
                .zip(&more.counts)
                .map(|(a, b)| a + b)
                .collect(),
        })
    }

    #[expect(clippy::cast_precision_loss, reason = "an estimate")]
    fn estimate_quantile(&self, q: f64) -> Option<f64> {
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

impl ExponentialBuckets {
    fn check_scale_and_size(&self) -> anyhow::Result<()> {
        ensure!(
            OTLP_EXPONENTIAL_SCALES.contains(&self.scale),
            "an exponential histogram has a scale from {} to {}, not {}",
            OTLP_EXPONENTIAL_SCALES.start(),
            OTLP_EXPONENTIAL_SCALES.end(),
            self.scale
        );
        ensure!(
            i64::try_from(self.bucket_count()).is_ok_and(|count| count <= MAX_SPANNED_BUCKETS),
            "an exponential histogram has at most {MAX_SPANNED_BUCKETS} buckets, not {}",
            self.bucket_count()
        );
        Ok(())
    }

    fn join_buckets_down_to_scale(&self, scale: i32) -> Self {
        let halvings = self.scale.saturating_sub(scale).max(0).unsigned_abs();
        Self {
            scale: self.scale.min(scale),
            zero_count: self.zero_count,
            positive: self.positive.join_neighbours(halvings),
            negative: self.negative.join_neighbours(halvings),
        }
    }

    fn add_counts_of(&self, more: &Self) -> Option<Self> {
        let scale = self.scale.min(more.scale);
        let sum = self.join_buckets_down_to_scale(scale);
        let more = more.join_buckets_down_to_scale(scale);
        Some(Self {
            scale,
            zero_count: sum.zero_count + more.zero_count,
            positive: sum.positive.add_counts_of(&more.positive)?,
            negative: sum.negative.add_counts_of(&more.negative)?,
        })
    }

    fn subtract_counts_of(&self, earlier: &Self) -> Option<Self> {
        let scale = self.scale.min(earlier.scale);
        let now = self.join_buckets_down_to_scale(scale);
        let then = earlier.join_buckets_down_to_scale(scale);
        Some(Self {
            scale,
            zero_count: now.zero_count.checked_sub(then.zero_count)?,
            positive: now.positive.subtract_counts_of(&then.positive)?,
            negative: now.negative.subtract_counts_of(&then.negative)?,
        })
    }

    const fn bucket_count(&self) -> usize {
        self.positive.counts.len() + self.negative.counts.len()
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "a bucket index is far below 2^53"
    )]
    fn lower_bound_of_bucket(&self, index: i64) -> f64 {
        (index as f64 * 2_f64.powi(-self.scale)).exp2()
    }

    fn to_explicit_buckets(&self) -> ExplicitBuckets {
        let mut bounds = Vec::new();
        let mut counts = Vec::new();
        if !self.negative.counts.is_empty() {
            bounds.push(-self.lower_bound_of_bucket(self.negative.end_index()));
            counts.push(0);
            for (index, count) in self.negative.counts_by_index().rev() {
                bounds.push(-self.lower_bound_of_bucket(index));
                counts.push(count);
            }
        }
        if !self.negative.counts.is_empty() || self.zero_count > 0 {
            bounds.push(0.0);
            counts.push(self.zero_count);
        }
        if !self.positive.counts.is_empty() {
            bounds.push(self.lower_bound_of_bucket(i64::from(self.positive.offset)));
            counts.push(0);
            for (index, count) in self.positive.counts_by_index() {
                bounds.push(self.lower_bound_of_bucket(index + 1));
                counts.push(count);
            }
        }
        counts.push(0);
        ExplicitBuckets { bounds, counts }
    }

    fn join_buckets_until_at_most(&self, max_bucket_count: usize) -> Self {
        let mut coarser = self.clone();
        while coarser.bucket_count() > max_bucket_count {
            coarser = coarser.join_buckets_down_to_scale(coarser.scale - 1);
        }
        coarser
    }
}

impl IndexedCounts {
    fn end_index(&self) -> i64 {
        i64::from(self.offset) + i64::try_from(self.counts.len()).unwrap_or(i64::MAX)
    }

    fn counts_by_index(&self) -> impl DoubleEndedIterator<Item = (i64, u64)> + '_ {
        let offset = i64::from(self.offset);
        self.counts
            .iter()
            .enumerate()
            .map(move |(slot, &count)| (offset + i64::try_from(slot).unwrap_or(i64::MAX), count))
    }

    fn count_at(&self, index: i64) -> u64 {
        usize::try_from(index - i64::from(self.offset))
            .ok()
            .and_then(|slot| self.counts.get(slot))
            .copied()
            .unwrap_or(0)
    }

    fn collect_counts_between(
        start_index: i64,
        end_index: i64,
        count_at: impl Fn(i64) -> Option<u64>,
    ) -> Option<Self> {
        if end_index - start_index > MAX_SPANNED_BUCKETS {
            return None;
        }
        Some(Self {
            offset: i32::try_from(start_index).ok()?,
            counts: (start_index..end_index)
                .map(count_at)
                .collect::<Option<_>>()?,
        })
    }

    // Halving the scale joins the buckets `2i` and `2i + 1` into bucket `i`.
    fn join_neighbours(&self, halvings: u32) -> Self {
        let mut joined: BTreeMap<i64, u64> = BTreeMap::new();
        for (index, count) in self.counts_by_index() {
            *joined.entry(index >> halvings).or_default() += count;
        }
        let Some((&start_index, _)) = joined.first_key_value() else {
            return Self::default();
        };
        let end_index = ((self.end_index() - 1) >> halvings) + 1;
        Self::collect_counts_between(start_index, end_index, |index| {
            Some(joined.get(&index).copied().unwrap_or(0))
        })
        .unwrap_or_default()
    }

    fn add_counts_of(&self, more: &Self) -> Option<Self> {
        if self.counts.is_empty() {
            return Some(more.clone());
        }
        if more.counts.is_empty() {
            return Some(self.clone());
        }
        Self::collect_counts_between(
            i64::from(self.offset.min(more.offset)),
            self.end_index().max(more.end_index()),
            |index| Some(self.count_at(index) + more.count_at(index)),
        )
    }

    fn subtract_counts_of(&self, earlier: &Self) -> Option<Self> {
        if earlier.counts.is_empty() {
            return Some(self.clone());
        }
        Self::collect_counts_between(
            i64::from(self.offset.min(earlier.offset)),
            self.end_index().max(earlier.end_index()),
            |index| self.count_at(index).checked_sub(earlier.count_at(index)),
        )
    }
}

/// The values a histogram series recorded in one time step: its points
/// merged, with the increases of cumulative points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Distribution {
    /// The upper bounds of the buckets. The last bucket has no upper bound.
    /// An exponential histogram gets the bounds of its buckets, joined until
    /// 64 of them are left.
    pub bounds: Vec<f64>,
    pub counts: Vec<u64>,
    pub count: u64,
    #[schema(required = true)]
    pub sum: Option<f64>,
    /// Estimates of the median and the 90th and 99th percentiles, by linear
    /// interpolation inside a bucket. `None` without values.
    #[schema(required = true)]
    pub p50: Option<f64>,
    #[schema(required = true)]
    pub p90: Option<f64>,
    #[schema(required = true)]
    pub p99: Option<f64>,
}

impl From<Histogram> for Distribution {
    fn from(histogram: Histogram) -> Self {
        let (estimated, shown) = match histogram.buckets {
            Buckets::Explicit(explicit) => (explicit.clone(), explicit),
            Buckets::Exponential(exponential) => (
                exponential.to_explicit_buckets(),
                exponential
                    .join_buckets_until_at_most(MAX_DISTRIBUTION_BUCKETS)
                    .to_explicit_buckets(),
            ),
        };
        Self {
            bounds: shown.bounds,
            counts: shown.counts,
            count: histogram.count,
            sum: histogram.sum,
            p50: estimated.estimate_quantile(0.5),
            p90: estimated.estimate_quantile(0.9),
            p99: estimated.estimate_quantile(0.99),
        }
    }
}

// Takes the points of one series, oldest first.
pub struct StepHistograms {
    temporality: Temporality,
    previous_point: Option<Histogram>,
    by_step: BTreeMap<i64, Histogram>,
}

impl StepHistograms {
    #[must_use]
    pub const fn new(temporality: Temporality) -> Self {
        Self {
            temporality,
            previous_point: None,
            by_step: BTreeMap::new(),
        }
    }

    pub fn add_point(&mut self, step: i64, point: Histogram) {
        let increase = match self.temporality {
            Temporality::Delta => point,
            Temporality::Cumulative => {
                let Some(previous) = self.previous_point.replace(point.clone()) else {
                    return;
                };
                // A count that fell is a histogram that started again from zero.
                point.increase_since(&previous).unwrap_or(point)
            }
        };
        match self.by_step.get_mut(&step) {
            Some(merged) => merged.add_increase(increase),
            None => {
                self.by_step.insert(step, increase);
            }
        }
    }

    #[must_use]
    pub fn into_histograms_by_step(self) -> BTreeMap<i64, Histogram> {
        self.by_step
    }
}
