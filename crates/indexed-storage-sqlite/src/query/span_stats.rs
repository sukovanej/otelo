use std::collections::{BTreeMap, HashMap};

use otelo_indexed_storage::query::{Latency, SpanBucket, SpanStats};

use super::timestamp_from_nanos;

// A value is taken for the middle of its bucket, 2γ^i/(γ+1), which is off by (γ-1)/(γ+1), about 1%.
const SKETCH_BUCKET_GROWTH: f64 = 1.02;

// Buckets that grow by a factor keep the relative error of a percentile, in memory that grows with the logarithm of the durations.
#[derive(Default)]
struct DurationSketch {
    zero_count: u64,
    counts_by_bucket_index: BTreeMap<i32, u64>,
}

impl DurationSketch {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "the index of a bucket is below 2000 for any i64"
    )]
    fn add_duration(&mut self, duration_ns: i64) {
        if duration_ns <= 0 {
            self.zero_count += 1;
        } else {
            let index = (duration_ns as f64).log(SKETCH_BUCKET_GROWTH).ceil() as i32;
            *self.counts_by_bucket_index.entry(index).or_default() += 1;
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "an estimate"
    )]
    fn nearest_rank_quantile(&self, quantile: f64, count: u64) -> i64 {
        let rank = ((quantile * count as f64).ceil() as u64).max(1) - 1;
        let mut seen = self.zero_count;
        if rank < seen {
            return 0;
        }
        for (&index, &bucket_count) in &self.counts_by_bucket_index {
            seen += bucket_count;
            if rank < seen {
                return (2.0 * SKETCH_BUCKET_GROWTH.powi(index) / (SKETCH_BUCKET_GROWTH + 1.0))
                    .round() as i64;
            }
        }
        0
    }
}

#[derive(Default)]
pub(super) struct SpanTally {
    count: u64,
    errors: u64,
    pub(super) total_ns: i64,
    sketch: DurationSketch,
}

impl SpanTally {
    pub(super) fn add_span(&mut self, duration_ns: i64, failed: bool) {
        self.count += 1;
        self.errors += u64::from(failed);
        self.total_ns = self.total_ns.saturating_add(duration_ns);
        self.sketch.add_duration(duration_ns);
    }

    pub(super) fn to_span_stats(&self) -> SpanStats {
        SpanStats {
            count: self.count,
            errors: self.errors,
            total_ns: self.total_ns,
            latency: (self.count > 0).then(|| Latency {
                p50: self.sketch.nearest_rank_quantile(0.5, self.count),
                p95: self.sketch.nearest_rank_quantile(0.95, self.count),
                p99: self.sketch.nearest_rank_quantile(0.99, self.count),
            }),
        }
    }
}

pub(super) fn fill_span_buckets(
    steps: &HashMap<i64, SpanTally>,
    first_step_at: i64,
    end_at: i64,
    step_ns: i64,
) -> Vec<SpanBucket> {
    let empty = SpanTally::default();
    (first_step_at..end_at)
        .step_by(usize::try_from(step_ns).unwrap_or(usize::MAX))
        .map(|start_at| SpanBucket {
            start_at: timestamp_from_nanos(start_at),
            spans: steps.get(&start_at).unwrap_or(&empty).to_span_stats(),
        })
        .collect()
}
