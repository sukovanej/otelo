use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::RecvTimeoutError;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::Context;
use otelo_indexed_storage::{
    Batch, FrameCounts, Indexing, PipelineMeters, RecordCounts, Records, SignalIndexing,
    now_unix_nanos,
};
use otelo_journal::{Frame, Frames, Hour, Journal, Position, SyncedEnd, SyncedEndInbox};
use otelo_query::Signal;
use tokio::sync::watch;

use crate::day::Day;
use crate::indexes::{Indexes, VersionedAttributes};
use crate::progress::Progress;
use crate::retention::{OldestRetainedDays, RetentionDays};
use crate::series::MAX_SERIES_PER_METRIC;
use crate::telemetry_file::{ResourceRecords, TelemetryFile, rows_of_points};

const REJECTION_REPORT_INTERVAL: Duration = Duration::from_mins(1);
const RETENTION_INTERVAL: Duration = Duration::from_hours(1);
const ROLLUP_INTERVAL: Duration = Duration::from_mins(1);
const MAX_FRAMES_PER_TRANSACTION: usize = 64;
const INDEX_CHECK_INTERVAL: Duration = Duration::from_secs(1);
// A frame the journal cannot read stays unreadable, and each retry would log it again.
const RETRY_AFTER_READ_FAILURE: Duration = Duration::from_mins(1);
// A burst of frames puts the index a moment behind, which is not worth telling the reader.
const MAX_LAG_OF_CAUGHT_UP_INDEX: Duration = Duration::from_secs(10);

pub type FrameMapper = Arc<dyn Fn(Signal, &[u8]) -> anyhow::Result<Batch> + Send + Sync>;

#[derive(Clone)]
pub struct Config {
    pub directory: PathBuf,
    pub retention_days: RetentionDays,
    pub indexes: Indexes,
}

impl Config {
    #[must_use]
    pub fn new(directory: PathBuf, retention_days: RetentionDays) -> Self {
        Self {
            directory,
            retention_days,
            indexes: Indexes::new(BTreeSet::new()),
        }
    }

    #[must_use]
    pub fn oldest_retained_days(&self, today: Day) -> OldestRetainedDays {
        self.retention_days.oldest_retained_days(today)
    }
}

pub struct Indexer {
    thread: JoinHandle<()>,
}

impl Indexer {
    pub fn spawn(
        config: Config,
        journal: Arc<dyn Journal>,
        synced_ends: SyncedEndInbox,
        map_frame: FrameMapper,
        meters: Arc<PipelineMeters>,
        indexing: watch::Sender<Indexing>,
    ) -> anyhow::Result<Self> {
        let state = IndexerState::open(config, journal, map_frame, meters, indexing)?;
        let thread = thread::Builder::new()
            .name("telemetry-indexer".into())
            .spawn(move || state.index_until_journal_stops(&synced_ends))
            .context("start the telemetry indexer")?;
        Ok(Self { thread })
    }

    pub fn join(self) -> anyhow::Result<()> {
        self.thread
            .join()
            .map_err(|_| anyhow::anyhow!("the telemetry indexer panicked"))
    }
}

pub fn index_journal_until_caught_up(
    config: Config,
    journal: Arc<dyn Journal>,
    map_frame: FrameMapper,
    meters: Arc<PipelineMeters>,
    mut report_indexed_hour: impl FnMut(Signal, Hour),
) -> anyhow::Result<()> {
    let mut state = IndexerState::open(
        config,
        journal,
        map_frame,
        meters,
        watch::Sender::new(Indexing::STARTING),
    )?;
    state.apply_index_changes();
    for signal in Signal::ALL {
        let mut reported_hour = None;
        loop {
            let progress = state.index_next_frames(signal)?;
            let indexed_hour = state
                .cursor(signal)
                .indexed_position
                .map(|position| position.segment_hour);
            if let Some(indexed_hour) = indexed_hour
                && reported_hour != Some(indexed_hour)
            {
                report_indexed_hour(signal, indexed_hour);
                reported_hour = Some(indexed_hour);
            }
            if progress == Progress::CaughtUp {
                break;
            }
        }
    }
    state.report_rejected_points();
    Ok(())
}

#[derive(Default)]
enum JournalRead {
    #[default]
    Idle,
    Reading(Frames),
    FailedAt(Instant),
}

#[derive(Clone, Copy, Default)]
enum IndexedUntil {
    #[default]
    NothingRead,
    FrameReceivedAt(i64),
    JournalEnd,
}

#[derive(Default)]
struct SignalCursor {
    indexed_position: Option<Position>,
    indexed_until: IndexedUntil,
    newest_journal_received_at: Option<i64>,
    catch_up_first_received_at: Option<i64>,
    journal_read: JournalRead,
}

impl SignalCursor {
    fn follow_catch_up(
        &mut self,
        index_lag: Duration,
        first_frame_received_at: i64,
        last_frame_received_at: i64,
        journal_received_until: i64,
    ) -> SignalIndexing {
        if index_lag <= MAX_LAG_OF_CAUGHT_UP_INDEX {
            self.catch_up_first_received_at = None;
            return SignalIndexing::CaughtUp;
        }
        let catch_up_first_received_at = *self
            .catch_up_first_received_at
            .get_or_insert(first_frame_received_at);
        SignalIndexing::CatchingUp {
            indexed_percent: measure_indexed_percent(
                catch_up_first_received_at,
                last_frame_received_at,
                journal_received_until,
            ),
        }
    }
}

struct IndexerState {
    config: Config,
    file: TelemetryFile,
    journal: Arc<dyn Journal>,
    map_frame: FrameMapper,
    meters: Arc<PipelineMeters>,
    indexing: watch::Sender<Indexing>,
    cursors: [SignalCursor; 3],
    applied_indexes: Option<VersionedAttributes>,
    rejected_points: u64,
    reported_rejected_points: u64,
}

impl IndexerState {
    fn open(
        config: Config,
        journal: Arc<dyn Journal>,
        map_frame: FrameMapper,
        meters: Arc<PipelineMeters>,
        indexing: watch::Sender<Indexing>,
    ) -> anyhow::Result<Self> {
        let file = TelemetryFile::open(&config.directory)?;
        let mut cursors: [SignalCursor; 3] = Default::default();
        for (signal, cursor) in Signal::ALL.into_iter().zip(&mut cursors) {
            cursor.indexed_position = file.read_indexed_position(signal)?;
        }
        Ok(Self {
            config,
            file,
            journal,
            map_frame,
            meters,
            indexing,
            cursors,
            applied_indexes: None,
            rejected_points: 0,
            reported_rejected_points: 0,
        })
    }

    const fn cursor(&self, signal: Signal) -> &SignalCursor {
        &self.cursors[signal_index(signal)]
    }

    const fn cursor_mut(&mut self, signal: Signal) -> &mut SignalCursor {
        &mut self.cursors[signal_index(signal)]
    }

    fn index_until_journal_stops(mut self, synced_ends: &SyncedEndInbox) {
        let mut due_signals: BTreeSet<Signal> = Signal::ALL.into_iter().collect();
        let mut next_report_at = Instant::now() + REJECTION_REPORT_INTERVAL;
        let mut next_retention_at = Instant::now();
        let mut next_rollup_at = Instant::now();
        let mut catch_up_started_at = Some(Instant::now());
        loop {
            self.apply_index_changes();
            due_signals
                .retain(|&signal| self.index_next_frames_or_log(signal) == Progress::MoreIsDue);
            if due_signals.is_empty()
                && !self.has_failed_indexing()
                && let Some(started_at) = catch_up_started_at.take()
            {
                tracing::info!(
                    seconds = started_at.elapsed().as_secs_f64(),
                    "caught up with the journal"
                );
            }
            let timeout = if due_signals.is_empty() {
                next_report_at
                    .min(next_retention_at)
                    .min(next_rollup_at)
                    .saturating_duration_since(Instant::now())
                    .min(INDEX_CHECK_INTERVAL)
            } else {
                Duration::ZERO
            };
            match synced_ends.wait_for_synced_end(timeout) {
                Ok(synced_end) => {
                    for synced_end in
                        std::iter::once(synced_end).chain(synced_ends.take_queued_synced_ends())
                    {
                        self.note_synced_end(synced_end);
                        due_signals.insert(synced_end.signal);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let now = Instant::now();
            if now >= next_report_at {
                self.report_rejected_points();
                next_report_at = now + REJECTION_REPORT_INTERVAL;
            }
            // A day of every signal expires at once, and the indexer takes frames in between.
            if now >= next_retention_at {
                next_retention_at = match self.delete_next_past_retention() {
                    Progress::MoreIsDue => now,
                    Progress::CaughtUp => now + RETENTION_INTERVAL,
                };
            }
            if now >= next_rollup_at {
                // A daemon that was down has hours to roll up, and takes frames in between.
                next_rollup_at = match self.roll_up_next_due_metrics() {
                    Progress::MoreIsDue => now,
                    Progress::CaughtUp => now + ROLLUP_INTERVAL,
                };
            }
        }
        // The journal stopped after its last sync, so what it synced is indexed before the end.
        for signal in Signal::ALL {
            while self.index_next_frames_or_log(signal) == Progress::MoreIsDue {}
        }
        self.report_rejected_points();
    }

    fn has_failed_indexing(&self) -> bool {
        self.cursors
            .iter()
            .any(|cursor| matches!(cursor.journal_read, JournalRead::FailedAt(_)))
    }

    fn note_synced_end(&mut self, synced_end: SyncedEnd) {
        let cursor = self.cursor_mut(synced_end.signal);
        cursor.newest_journal_received_at = cursor
            .newest_journal_received_at
            .max(Some(synced_end.newest_received_at));
    }

    fn index_next_frames_or_log(&mut self, signal: Signal) -> Progress {
        if let JournalRead::FailedAt(failed_at) = self.cursor(signal).journal_read
            && failed_at.elapsed() < RETRY_AFTER_READ_FAILURE
        {
            return Progress::CaughtUp;
        }
        self.index_next_frames(signal).unwrap_or_else(|error| {
            tracing::error!(signal = signal.name(), "index the journal: {error:#}");
            self.cursor_mut(signal).journal_read = JournalRead::FailedAt(Instant::now());
            Progress::CaughtUp
        })
    }

    fn index_next_frames(&mut self, signal: Signal) -> anyhow::Result<Progress> {
        let oldest_retained_at = self
            .config
            .oldest_retained_days(Day::today())
            .of_signal(signal)
            .start_at();
        let mut frames = match std::mem::take(&mut self.cursor_mut(signal).journal_read) {
            JournalRead::Reading(frames) => frames,
            JournalRead::Idle | JournalRead::FailedAt(_) => {
                let from = choose_resume_position(
                    self.cursor(signal).indexed_position,
                    oldest_retained_at,
                );
                self.journal.read_frames(signal, Some(from))?
            }
        };
        let read_frames = frames
            .by_ref()
            .take(MAX_FRAMES_PER_TRANSACTION)
            .collect::<anyhow::Result<Vec<Frame>>>()?;
        let caught_up = read_frames.len() < MAX_FRAMES_PER_TRANSACTION;
        let Some(last_frame) = read_frames.last() else {
            self.meters.set_index_lag(signal, Duration::ZERO);
            let cursor = self.cursor_mut(signal);
            cursor.indexed_until = IndexedUntil::JournalEnd;
            cursor.catch_up_first_received_at = None;
            self.report_signal_indexing(signal, SignalIndexing::CaughtUp);
            return Ok(Progress::CaughtUp);
        };
        let transaction_started_at = Instant::now();
        let mut frame_counts = FrameCounts::default();
        let mut batches = Vec::new();
        for frame in &read_frames {
            if frame.received_at < oldest_retained_at {
                frame_counts.skipped += 1;
                continue;
            }
            match (self.map_frame)(signal, &frame.request) {
                Ok(batch) => {
                    frame_counts.indexed += 1;
                    batches.push((frame.received_at, batch));
                }
                Err(error) => {
                    frame_counts.undecodable += 1;
                    tracing::warn!(
                        signal = signal.name(),
                        "skip a journal frame that does not decode: {error:#}"
                    );
                }
            }
        }
        let (resource_records, mut record_counts) =
            self.select_retained_records(batches.iter().flat_map(|(received_at, batch)| {
                batch.iter().map(move |records| (*received_at, records))
            }));
        let rejected_points =
            self.file
                .write_indexed_frames(&resource_records, signal, last_frame.position_after)?;
        record_counts.written -= rejected_points;
        record_counts.rejected = rejected_points;
        self.rejected_points += rejected_points;
        let meters = &self.meters;
        meters.add_frame_counts(signal, frame_counts);
        meters.add_record_counts(signal, record_counts);
        meters.record_index_transaction(signal, transaction_started_at.elapsed());
        let cursor = &mut self.cursors[signal_index(signal)];
        cursor.indexed_position = Some(last_frame.position_after);
        cursor.indexed_until = if caught_up {
            IndexedUntil::JournalEnd
        } else {
            IndexedUntil::FrameReceivedAt(last_frame.received_at)
        };
        let journal_received_until = cursor
            .newest_journal_received_at
            .unwrap_or_else(now_unix_nanos);
        let index_lag = if caught_up {
            Duration::ZERO
        } else {
            Duration::from_nanos(
                u64::try_from(journal_received_until - last_frame.received_at).unwrap_or(0),
            )
        };
        meters.set_index_lag(signal, index_lag);
        let signal_indexing = cursor.follow_catch_up(
            index_lag,
            read_frames[0].received_at,
            last_frame.received_at,
            journal_received_until,
        );
        if !caught_up {
            cursor.journal_read = JournalRead::Reading(frames);
        }
        self.report_signal_indexing(signal, signal_indexing);
        Ok(if caught_up {
            Progress::CaughtUp
        } else {
            Progress::MoreIsDue
        })
    }

    fn report_signal_indexing(&self, signal: Signal, signal_indexing: SignalIndexing) {
        self.indexing
            .send_if_modified(|indexing| indexing.replace_signal(signal, signal_indexing));
    }

    fn select_retained_records<'a>(
        &self,
        records: impl Iterator<Item = (i64, &'a Records)>,
    ) -> (Vec<ResourceRecords<'a>>, RecordCounts) {
        let today = Day::today();
        let oldest_retained_days = self.config.oldest_retained_days(today);
        // A record from far ahead would stay past the retention of its signal.
        let end_of_tomorrow_at = today.add_days(2).start_at();
        let is_retained = |signal: Signal, recorded_at: i64| {
            recorded_at >= oldest_retained_days.of_signal(signal).start_at()
                && recorded_at < end_of_tomorrow_at
        };
        let mut record_counts = RecordCounts::default();
        let mut resource_records = Vec::new();
        for (received_at, records) in records {
            let mut retained = ResourceRecords {
                received_at,
                resource: &records.resource,
                logs: Vec::new(),
                spans: Vec::new(),
                point_rows_by_metric: Vec::new(),
            };
            for log in &records.logs {
                if is_retained(Signal::Logs, log.logged_at) {
                    retained.logs.push(log);
                } else {
                    record_counts.skipped += 1;
                }
            }
            for span in &records.spans {
                if is_retained(Signal::Spans, span.started_at) {
                    retained.spans.push(span);
                } else {
                    record_counts.skipped += 1;
                }
            }
            for metric in &records.metrics {
                let mut point_rows = rows_of_points(&metric.points);
                let point_count = point_rows.len();
                point_rows
                    .retain(|point_row| is_retained(Signal::Metrics, point_row.recorded_at()));
                record_counts.skipped += (point_count - point_rows.len()) as u64;
                if !point_rows.is_empty() {
                    retained.point_rows_by_metric.push((metric, point_rows));
                }
            }
            record_counts.written += retained.record_count();
            if !retained.is_empty() {
                resource_records.push(retained);
            }
        }
        (resource_records, record_counts)
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

    fn report_rejected_points(&mut self) {
        if self.rejected_points > self.reported_rejected_points {
            tracing::warn!(
                rejected = self.rejected_points - self.reported_rejected_points,
                "a metric has more than {MAX_SERIES_PER_METRIC} series, and the points of its \
                 newer series were rejected"
            );
            self.reported_rejected_points = self.rejected_points;
        }
    }

    // A minute rolled up before its points are indexed would stay without them.
    fn roll_up_next_due_metrics(&mut self) -> Progress {
        let indexed_until_at = match self.cursor(Signal::Metrics).indexed_until {
            IndexedUntil::NothingRead => return Progress::CaughtUp,
            IndexedUntil::FrameReceivedAt(received_at) => received_at,
            IndexedUntil::JournalEnd => now_unix_nanos(),
        };
        let oldest_point_at = self
            .config
            .oldest_retained_days(Day::today())
            .metrics
            .start_at();
        self.file
            .roll_up_next_due(indexed_until_at, oldest_point_at)
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

// The journal keeps longer than the index, and its hours before the retention hold nothing to
// index, so they are not read.
fn choose_resume_position(indexed_position: Option<Position>, oldest_retained_at: i64) -> Position {
    let oldest_retained_position = Position {
        segment_hour: Hour::containing(oldest_retained_at),
        byte_offset: 0,
    };
    indexed_position
        .filter(|position| *position >= oldest_retained_position)
        .unwrap_or(oldest_retained_position)
}

fn measure_indexed_percent(
    catch_up_first_received_at: i64,
    indexed_received_at: i64,
    journal_received_until: i64,
) -> u8 {
    let indexed_nanos = i128::from(indexed_received_at - catch_up_first_received_at);
    let catch_up_nanos = i128::from(journal_received_until - catch_up_first_received_at).max(1);
    // A catch-up that reached 100 % has caught up.
    let indexed_percent = (indexed_nanos * 100 / catch_up_nanos).clamp(0, 99);
    u8::try_from(indexed_percent).expect("a percent fits a u8")
}

const fn signal_index(signal: Signal) -> usize {
    match signal {
        Signal::Logs => 0,
        Signal::Spans => 1,
        Signal::Metrics => 2,
    }
}
