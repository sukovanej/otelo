use std::collections::BTreeSet;
use std::num::NonZeroU16;
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::Context;
use otelo_indexed_storage::{
    Attributes, BatchInbox, Metric, NumberPoint, Points, Records, Resource, Temporality,
};
use otelo_query::Signal;

use crate::day::Day;
use crate::indexes::{Indexes, VersionedAttributes};
use crate::retention::OldestRetainedDays;
use crate::rollup::Progress;
use crate::series::MAX_SERIES_PER_METRIC;
use crate::telemetry_file::{PointRow, ResourceRecords, TelemetryFile, rows_of_points};

const DEFAULT_RETENTION_DAYS: NonZeroU16 = NonZeroU16::new(7).expect("seven is not zero");
const LOSS_REPORT_INTERVAL: Duration = Duration::from_mins(1);
const RETENTION_INTERVAL: Duration = Duration::from_hours(1);
const ROLLUP_INTERVAL: Duration = Duration::from_mins(1);
const MAX_BATCHES_PER_TRANSACTION: usize = 64;
const INDEX_CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct Config {
    pub directory: PathBuf,
    pub logs_retention_days: NonZeroU16,
    pub traces_retention_days: NonZeroU16,
    pub metrics_retention_days: NonZeroU16,
    pub indexes: Indexes,
}

impl Config {
    #[must_use]
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            logs_retention_days: DEFAULT_RETENTION_DAYS,
            traces_retention_days: DEFAULT_RETENTION_DAYS,
            metrics_retention_days: DEFAULT_RETENTION_DAYS,
            indexes: Indexes::new(BTreeSet::new()),
        }
    }

    #[must_use]
    pub fn oldest_retained_days(&self, today: Day) -> OldestRetainedDays {
        let oldest_retained_day =
            |retention_days: NonZeroU16| today.add_days(1 - i64::from(retention_days.get()));
        OldestRetainedDays {
            logs: oldest_retained_day(self.logs_retention_days),
            spans: oldest_retained_day(self.traces_retention_days),
            metrics: oldest_retained_day(self.metrics_retention_days),
        }
    }
}

pub struct Writer {
    thread: JoinHandle<()>,
}

impl Writer {
    pub fn spawn(config: Config, inbox: BatchInbox) -> anyhow::Result<Self> {
        let file = TelemetryFile::open(&config.directory)?;
        let thread = thread::Builder::new()
            .name("telemetry-writer".into())
            .spawn(move || WriterState::new(config, file).write_batches_until_disconnected(&inbox))
            .context("start the telemetry writer")?;
        Ok(Self { thread })
    }

    pub fn join(self) -> anyhow::Result<()> {
        self.thread
            .join()
            .map_err(|_| anyhow::anyhow!("the telemetry writer panicked"))
    }
}

struct WriterState {
    config: Config,
    file: TelemetryFile,
    reported_dropped_batches: u64,
    rejected_points: u64,
    reported_rejected_points: u64,
    applied_indexes: Option<VersionedAttributes>,
}

impl WriterState {
    const fn new(config: Config, file: TelemetryFile) -> Self {
        Self {
            config,
            file,
            reported_dropped_batches: 0,
            rejected_points: 0,
            reported_rejected_points: 0,
            applied_indexes: None,
        }
    }

    fn write_batches_until_disconnected(mut self, inbox: &BatchInbox) {
        // The writer takes no batch before it deleted what it no longer keeps.
        while self.delete_next_past_retention() == Progress::MoreIsDue {}
        let mut next_report_at = Instant::now() + LOSS_REPORT_INTERVAL;
        let mut next_retention_at = Instant::now() + RETENTION_INTERVAL;
        let mut next_rollup_at = Instant::now();
        loop {
            self.apply_index_changes();
            let timeout = next_report_at
                .min(next_retention_at)
                .min(next_rollup_at)
                .saturating_duration_since(Instant::now())
                .min(INDEX_CHECK_INTERVAL);
            match inbox.wait_for_batch(timeout) {
                Ok(batch) => {
                    let mut batches = vec![batch];
                    batches.extend(
                        inbox
                            .take_queued_batches()
                            .take(MAX_BATCHES_PER_TRANSACTION - 1),
                    );
                    self.write_records(batches.iter().flatten());
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let now = Instant::now();
            if now >= next_report_at {
                self.report_lost_telemetry(inbox.dropped_batches());
                next_report_at = now + LOSS_REPORT_INTERVAL;
            }
            // A lowered retention has much to delete, and the writer takes batches in between.
            if now >= next_retention_at {
                next_retention_at = match self.delete_next_past_retention() {
                    Progress::MoreIsDue => now,
                    Progress::CaughtUp => now + RETENTION_INTERVAL,
                };
            }
            if now >= next_rollup_at {
                // A daemon that was down has hours to roll up, and takes batches in between.
                next_rollup_at = match self.roll_up_next_due_metrics() {
                    Progress::MoreIsDue => now,
                    Progress::CaughtUp => now + ROLLUP_INTERVAL,
                };
            }
        }
        self.report_lost_telemetry(inbox.dropped_batches());
    }

    fn write_records<'a>(&mut self, records: impl Iterator<Item = &'a Records>) {
        let today = Day::today();
        let oldest_retained_days = self.config.oldest_retained_days(today);
        // A record from far ahead would stay past the retention of its signal.
        let first_day_past_retention = today.add_days(2).start_at();
        let is_retained = |signal: Signal, recorded_at: i64| {
            recorded_at >= oldest_retained_days.of_signal(signal).start_at()
                && recorded_at < first_day_past_retention
        };
        let mut skipped_records = 0;
        let mut resource_records = Vec::new();
        for records in records {
            let mut retained = ResourceRecords {
                resource: &records.resource,
                logs: Vec::new(),
                spans: Vec::new(),
                point_rows_by_metric: Vec::new(),
            };
            for log in &records.logs {
                if is_retained(Signal::Logs, log.logged_at) {
                    retained.logs.push(log);
                } else {
                    skipped_records += 1;
                }
            }
            for span in &records.spans {
                if is_retained(Signal::Spans, span.started_at) {
                    retained.spans.push(span);
                } else {
                    skipped_records += 1;
                }
            }
            for metric in &records.metrics {
                let mut point_rows = rows_of_points(&metric.points);
                let point_count = point_rows.len();
                point_rows
                    .retain(|point_row| is_retained(Signal::Metrics, point_row.recorded_at()));
                skipped_records += point_count - point_rows.len();
                if !point_rows.is_empty() {
                    retained.point_rows_by_metric.push((metric, point_rows));
                }
            }
            if !retained.is_empty() {
                resource_records.push(retained);
            }
        }
        if skipped_records > 0 {
            tracing::debug!(
                skipped = skipped_records,
                "skipped records outside the retention"
            );
        }
        if resource_records.is_empty() {
            return;
        }
        match self.file.write_records(&resource_records) {
            Ok(rejected_points) => self.rejected_points += rejected_points,
            Err(error) => tracing::error!("write telemetry: {error:#}"),
        }
    }

    fn apply_index_changes(&mut self) {
        let wanted_indexes = self.config.indexes.versioned_attributes();
        if self
            .applied_indexes
            .as_ref()
            .is_some_and(|applied| applied.version == wanted_indexes.version)
        {
            return;
        }
        if let Err(error) = self.file.apply_indexes(&wanted_indexes.attributes) {
            tracing::error!("index the attributes: {error:#}");
        }
        self.applied_indexes = Some(wanted_indexes);
    }

    fn report_lost_telemetry(&mut self, dropped_batches: u64) {
        if dropped_batches > self.reported_dropped_batches {
            tracing::warn!(
                dropped = dropped_batches - self.reported_dropped_batches,
                "the telemetry channel was full and dropped batches"
            );
            self.reported_dropped_batches = dropped_batches;
        }
        if self.rejected_points > self.reported_rejected_points {
            tracing::warn!(
                rejected = self.rejected_points - self.reported_rejected_points,
                "a metric has more than {MAX_SERIES_PER_METRIC} series, and the points of its \
                 newer series were rejected"
            );
            self.reported_rejected_points = self.rejected_points;
        }
        let recorded_at = otelo_indexed_storage::now_unix_nanos();
        #[expect(clippy::cast_precision_loss, reason = "a count below 2^53")]
        let loss_counters = [
            (
                "otelo.telemetry.dropped_batches",
                "{batch}",
                dropped_batches,
            ),
            (
                "otelo.telemetry.rejected_points",
                "{point}",
                self.rejected_points,
            ),
        ]
        .map(|(name, unit, total)| Metric {
            name: name.into(),
            unit: unit.into(),
            attributes: Attributes::new(),
            points: Points::Counter(
                Temporality::Cumulative,
                vec![NumberPoint {
                    recorded_at,
                    value: total as f64,
                }],
            ),
        });
        let otelo_resource = Resource {
            service: "otelo".into(),
            attributes: Attributes::new(),
        };
        let records = ResourceRecords {
            resource: &otelo_resource,
            logs: Vec::new(),
            spans: Vec::new(),
            point_rows_by_metric: loss_counters
                .iter()
                .map(|loss_counter| {
                    let point_rows: Vec<PointRow> = rows_of_points(&loss_counter.points);
                    (loss_counter, point_rows)
                })
                .collect(),
        };
        if let Err(error) = self.file.write_records(&[records]) {
            tracing::error!("write the counters of lost telemetry: {error:#}");
        }
    }

    fn roll_up_next_due_metrics(&mut self) -> Progress {
        let oldest_point_at = self
            .config
            .oldest_retained_days(Day::today())
            .metrics
            .start_at();
        self.file
            .roll_up_next_due(otelo_indexed_storage::now_unix_nanos(), oldest_point_at)
            .unwrap_or_else(|error| {
                tracing::error!("roll up the metrics: {error:#}");
                Progress::CaughtUp
            })
    }

    fn delete_next_past_retention(&mut self) -> Progress {
        let oldest_retained_days = self.config.oldest_retained_days(Day::today());
        self.file
            .delete_next_past_retention(oldest_retained_days)
            .unwrap_or_else(|error| {
                tracing::error!("delete the telemetry past the retention: {error:#}");
                self.file.forget_cached_rows();
                Progress::CaughtUp
            })
    }
}
