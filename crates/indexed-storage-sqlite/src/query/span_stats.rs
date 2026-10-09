use std::collections::HashMap;

use otelo_indexed_storage::query::{Latency, SpanBucket, SpanStats};

use super::timestamp_from_nanos;
use crate::span_summary::{DurationHistogram, SpanSummary};

#[derive(Default)]
pub(super) struct SpanTally {
    count: u64,
    errors: u64,
    pub(super) total_ns: i64,
    durations: DurationHistogram,
}

impl SpanTally {
    pub(super) fn add_span(&mut self, duration_ns: i64, failed: bool) {
        self.count += 1;
        self.errors += u64::from(failed);
        self.total_ns = self.total_ns.saturating_add(duration_ns);
        self.durations.add_duration(duration_ns);
    }

    pub(super) fn add_summary(&mut self, summary: &SpanSummary, failed: bool) {
        self.count += summary.span_count;
        if failed {
            self.errors += summary.span_count;
        }
        self.total_ns = self.total_ns.saturating_add(summary.duration_sum_ns);
        self.durations.add_histogram(&summary.duration_histogram);
    }

    pub(super) fn to_span_stats(&self) -> SpanStats {
        SpanStats {
            count: self.count,
            errors: self.errors,
            total_ns: self.total_ns,
            latency: (self.count > 0).then(|| Latency {
                p50: self.durations.nearest_rank_quantile(0.5, self.count),
                p95: self.durations.nearest_rank_quantile(0.95, self.count),
                p99: self.durations.nearest_rank_quantile(0.99, self.count),
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
