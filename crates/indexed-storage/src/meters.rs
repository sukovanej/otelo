use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use otelo_query::Signal;

pub const DURATION_BUCKET_BOUNDS_SECONDS: [f64; 12] = [
    0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
];

const DURATION_BUCKET_COUNT: usize = DURATION_BUCKET_BOUNDS_SECONDS.len() + 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequestCounts {
    pub journaled: u64,
    pub refused: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCounts {
    pub indexed: u64,
    pub skipped: u64,
    pub undecodable: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecordCounts {
    pub written: u64,
    pub skipped: u64,
    pub rejected: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DurationCounts {
    pub bucket_counts: [u64; DURATION_BUCKET_COUNT],
    pub sum: Duration,
}

impl DurationCounts {
    #[must_use]
    pub fn count(&self) -> u64 {
        self.bucket_counts.iter().sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalReading {
    pub signal: Signal,
    pub requests: RequestCounts,
    pub journaled_bytes: u64,
    pub journal_sync_waits: DurationCounts,
    pub frames: FrameCounts,
    pub records: RecordCounts,
    pub index_transactions: DurationCounts,
    pub index_lag: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineReading {
    pub signals: [SignalReading; 3],
}

#[derive(Default)]
pub struct PipelineMeters {
    logs: SignalMeters,
    spans: SignalMeters,
    metrics: SignalMeters,
}

#[derive(Default)]
struct SignalMeters {
    journaled_requests: AtomicU64,
    refused_requests: AtomicU64,
    journaled_bytes: AtomicU64,
    journal_sync_waits: DurationHistogram,
    indexed_frames: AtomicU64,
    skipped_frames: AtomicU64,
    undecodable_frames: AtomicU64,
    written_records: AtomicU64,
    skipped_records: AtomicU64,
    rejected_records: AtomicU64,
    index_transactions: DurationHistogram,
    index_lag_ns: AtomicU64,
}

#[derive(Default)]
struct DurationHistogram {
    bucket_counts: [AtomicU64; DURATION_BUCKET_COUNT],
    sum_ns: AtomicU64,
}

impl DurationHistogram {
    fn record_duration(&self, duration: Duration) {
        let seconds = duration.as_secs_f64();
        let bucket = DURATION_BUCKET_BOUNDS_SECONDS
            .iter()
            .position(|&bound| seconds <= bound)
            .unwrap_or(DURATION_BUCKET_BOUNDS_SECONDS.len());
        self.bucket_counts[bucket].fetch_add(1, Ordering::Relaxed);
        self.sum_ns.fetch_add(nanos_of(duration), Ordering::Relaxed);
    }

    fn read_counts(&self) -> DurationCounts {
        DurationCounts {
            bucket_counts: self
                .bucket_counts
                .each_ref()
                .map(|count| count.load(Ordering::Relaxed)),
            sum: Duration::from_nanos(self.sum_ns.load(Ordering::Relaxed)),
        }
    }
}

impl PipelineMeters {
    pub fn count_journaled_request(&self, signal: Signal, request_bytes: u64) {
        let meters = self.of_signal(signal);
        meters.journaled_requests.fetch_add(1, Ordering::Relaxed);
        meters
            .journaled_bytes
            .fetch_add(request_bytes, Ordering::Relaxed);
    }

    pub fn count_refused_request(&self, signal: Signal) {
        self.of_signal(signal)
            .refused_requests
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_journal_sync_wait(&self, signal: Signal, wait: Duration) {
        self.of_signal(signal)
            .journal_sync_waits
            .record_duration(wait);
    }

    pub fn add_frame_counts(&self, signal: Signal, frames: FrameCounts) {
        let meters = self.of_signal(signal);
        meters
            .indexed_frames
            .fetch_add(frames.indexed, Ordering::Relaxed);
        meters
            .skipped_frames
            .fetch_add(frames.skipped, Ordering::Relaxed);
        meters
            .undecodable_frames
            .fetch_add(frames.undecodable, Ordering::Relaxed);
    }

    pub fn add_record_counts(&self, signal: Signal, records: RecordCounts) {
        let meters = self.of_signal(signal);
        meters
            .written_records
            .fetch_add(records.written, Ordering::Relaxed);
        meters
            .skipped_records
            .fetch_add(records.skipped, Ordering::Relaxed);
        meters
            .rejected_records
            .fetch_add(records.rejected, Ordering::Relaxed);
    }

    pub fn record_index_transaction(&self, signal: Signal, duration: Duration) {
        self.of_signal(signal)
            .index_transactions
            .record_duration(duration);
    }

    pub fn set_index_lag(&self, signal: Signal, lag: Duration) {
        self.of_signal(signal)
            .index_lag_ns
            .store(nanos_of(lag), Ordering::Relaxed);
    }

    #[must_use]
    pub fn read_pipeline(&self) -> PipelineReading {
        PipelineReading {
            signals: Signal::ALL.map(|signal| self.read_signal(signal)),
        }
    }

    fn read_signal(&self, signal: Signal) -> SignalReading {
        let meters = self.of_signal(signal);
        let load = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        SignalReading {
            signal,
            requests: RequestCounts {
                journaled: load(&meters.journaled_requests),
                refused: load(&meters.refused_requests),
            },
            journaled_bytes: load(&meters.journaled_bytes),
            journal_sync_waits: meters.journal_sync_waits.read_counts(),
            frames: FrameCounts {
                indexed: load(&meters.indexed_frames),
                skipped: load(&meters.skipped_frames),
                undecodable: load(&meters.undecodable_frames),
            },
            records: RecordCounts {
                written: load(&meters.written_records),
                skipped: load(&meters.skipped_records),
                rejected: load(&meters.rejected_records),
            },
            index_transactions: meters.index_transactions.read_counts(),
            index_lag: Duration::from_nanos(load(&meters.index_lag_ns)),
        }
    }

    const fn of_signal(&self, signal: Signal) -> &SignalMeters {
        match signal {
            Signal::Logs => &self.logs,
            Signal::Spans => &self.spans,
            Signal::Metrics => &self.metrics,
        }
    }
}

fn nanos_of(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}
