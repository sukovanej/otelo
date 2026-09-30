use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use std::{fs, io, ptr, thread};

use anyhow::Context;
use otelo_storage::{
    AttributeValue, Attributes, HistogramPoint, Inbox, IndexedAttribute, Log, Metric,
    MetricRetention, NumberPoint, Points, Records, Resource, Span, Temporality,
};
use rusqlite::{Connection, Transaction, params};

use crate::catalog::{CatalogCache, CatalogDelta, KeyGroup};
use crate::day::Day;
use crate::indexes::{Indexes, VersionedAttributes, apply_indexes_to_day_file};
use crate::rollup::{RollupProgress, Rollups};
use crate::series::{
    MAX_SERIES_PER_METRIC, SeriesCache, SeriesIdentity, StoredResource, StoredSeries,
    find_or_insert_resource_id, find_or_insert_series_id,
};
use crate::stored_schema::{StoredSchema, read_stored_schema};

const SCHEMA: &str = include_str!("schema.sql");

// A day file of another version keeps tables this code cannot read or write.
const SCHEMA_VERSION: i32 = 1;

const NO_ATTRIBUTES: &BTreeSet<IndexedAttribute> = &BTreeSet::new();

const LOSS_REPORT_INTERVAL: Duration = Duration::from_mins(1);
const RETENTION_INTERVAL: Duration = Duration::from_hours(1);
const ROLLUP_INTERVAL: Duration = Duration::from_mins(1);
const MAX_BATCHES_PER_TRANSACTION: usize = 64;
const INDEX_CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct Config {
    pub dir: PathBuf,
    pub retention_days: u16,
    pub minute_rollup_retention_days: u16,
    pub hour_rollup_retention_days: u16,
    pub indexes: Indexes,
}

impl Config {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            retention_days: 7,
            minute_rollup_retention_days: 14,
            hour_rollup_retention_days: 90,
            indexes: Indexes::new(BTreeSet::new()),
        }
    }

    pub(crate) fn oldest_retained_day(&self, today: Day) -> Day {
        today.plus(1 - i64::from(self.retention_days.max(1)))
    }

    pub(crate) fn metric_retention(&self, today: Day) -> MetricRetention {
        let oldest_day_of = |days: u16| today.plus(1 - i64::from(days.max(1))).start();
        MetricRetention {
            oldest_raw_at: oldest_day_of(self.retention_days),
            oldest_minute_at: oldest_day_of(self.minute_rollup_retention_days),
            oldest_hour_at: oldest_day_of(self.hour_rollup_retention_days),
        }
    }
}

pub struct Writer {
    thread: JoinHandle<()>,
}

impl Writer {
    pub fn spawn(config: Config, inbox: Inbox) -> anyhow::Result<Self> {
        fs::create_dir_all(&config.dir)
            .with_context(|| format!("make the telemetry directory {}", config.dir.display()))?;
        // No reader has a day file open yet, so a file can move.
        set_aside_day_files_of_another_schema(&config.dir)?;
        let thread = thread::Builder::new()
            .name("telemetry-writer".into())
            .spawn(move || State::new(config).run(&inbox))
            .context("start the telemetry writer")?;
        Ok(Self { thread })
    }

    pub fn join(self) -> anyhow::Result<()> {
        self.thread
            .join()
            .map_err(|_| anyhow::anyhow!("the telemetry writer panicked"))
    }
}

struct State {
    config: Config,
    files: HashMap<Day, DayFile>,
    reported_dropped_batches: u64,
    rejected_points: u64,
    reported_rejected_points: u64,
    applied_indexes: Option<VersionedAttributes>,
    // A rollup file that does not open must not stop the writer.
    rollups: Option<Rollups>,
}

impl State {
    fn new(config: Config) -> Self {
        let rollups = Rollups::open(&config.dir)
            .inspect_err(|error| tracing::error!("no metric rollups: {error:#}"))
            .ok();
        Self {
            rollups,
            config,
            files: HashMap::new(),
            reported_dropped_batches: 0,
            rejected_points: 0,
            reported_rejected_points: 0,
            applied_indexes: None,
        }
    }

    fn run(mut self, inbox: &Inbox) {
        self.apply_retention();
        let mut next_report = Instant::now() + LOSS_REPORT_INTERVAL;
        let mut next_retain = Instant::now() + RETENTION_INTERVAL;
        let mut next_rollup = Instant::now();
        loop {
            self.apply_index_changes();
            let wait = next_report
                .min(next_retain)
                .min(next_rollup)
                .saturating_duration_since(Instant::now())
                .min(INDEX_CHECK_INTERVAL);
            match inbox.recv_timeout(wait) {
                Ok(batch) => {
                    let mut batches = vec![batch];
                    batches.extend(inbox.try_iter().take(MAX_BATCHES_PER_TRANSACTION - 1));
                    self.write_records(batches.iter().flatten());
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let now = Instant::now();
            if now >= next_report {
                self.report_lost_telemetry(inbox.dropped_batches());
                next_report = now + LOSS_REPORT_INTERVAL;
            }
            if now >= next_retain {
                self.apply_retention();
                next_retain = now + RETENTION_INTERVAL;
            }
            if now >= next_rollup {
                // A daemon that was down has hours to roll up, and takes batches in between.
                next_rollup = match self.roll_up_next_due_metrics() {
                    RollupProgress::MoreIsDue => now,
                    RollupProgress::CaughtUp => now + ROLLUP_INTERVAL,
                };
            }
        }
        self.report_lost_telemetry(inbox.dropped_batches());
    }

    fn write_records<'a>(&mut self, records: impl Iterator<Item = &'a Records>) {
        let today = Day::today();
        // Retention would delete an older file, and a file more than a day ahead would outlive it.
        let retained_days = self.config.oldest_retained_day(today)..=today.plus(1);
        let mut parts: BTreeMap<(Day, usize), ResourceDayRecords<'a>> = BTreeMap::new();
        let mut resources = Vec::new();
        for (index, records) in records.enumerate() {
            resources.push(&records.resource);
            for log in &records.logs {
                let key = (Day::of(log.logged_at), index);
                parts.entry(key).or_default().logs.push(log);
            }
            for span in &records.spans {
                let key = (Day::of(span.started_at), index);
                parts.entry(key).or_default().spans.push(span);
            }
            for metric in &records.metrics {
                for point in rows_of_points(&metric.points) {
                    let key = (Day::of(point.recorded_at()), index);
                    parts.entry(key).or_default().push_point(metric, point);
                }
            }
        }
        let mut days: BTreeMap<Day, Vec<(&Resource, ResourceDayRecords)>> = BTreeMap::new();
        let mut skipped = 0;
        for ((day, index), part) in parts {
            if retained_days.contains(&day) {
                days.entry(day).or_default().push((resources[index], part));
            } else {
                skipped += part.len();
            }
        }
        if skipped > 0 {
            tracing::debug!(skipped, "skipped records outside the retention");
        }
        for (day, parts) in days {
            match self.write_day(day, &parts) {
                Ok(rejected_points) => self.rejected_points += rejected_points,
                Err(error) => tracing::error!(%day, "write telemetry: {error:#}"),
            }
        }
    }

    fn write_day(
        &mut self,
        day: Day,
        parts: &[(&Resource, ResourceDayRecords)],
    ) -> anyhow::Result<u64> {
        let file = match self.files.entry(day) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(DayFile::open(
                &self.config.dir,
                day,
                self.applied_indexes
                    .as_ref()
                    .map_or(NO_ATTRIBUTES, |applied| &applied.attributes),
            )?),
        };
        let result = file.write(parts);
        if result.is_err() {
            // The rollback took back what the caches learned.
            file.resource_ids_by_hash.clear();
            file.series = SeriesCache::default();
            file.catalog = CatalogCache::load(&file.conn).unwrap_or_default();
        }
        result
    }

    fn apply_index_changes(&mut self) {
        let wanted = self.config.indexes.versioned_attributes();
        if self
            .applied_indexes
            .as_ref()
            .is_some_and(|applied| applied.version == wanted.version)
        {
            return;
        }
        let names = match fs::read_dir(&self.config.dir) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                .filter(|name| name.ends_with(".sqlite"))
                .filter_map(|name| Day::from_file_name(&name))
                .collect::<Vec<_>>(),
            Err(error) => {
                tracing::error!("list the day files: {error:#}");
                return;
            }
        };
        for day in names {
            let result = match self.files.get(&day) {
                Some(file) => apply_indexes_to_day_file(&file.conn, &wanted.attributes)
                    .map_err(anyhow::Error::from),
                // A file from an older otelo may lack the newer tables.
                None => Connection::open(self.config.dir.join(day.file_name()))
                    .and_then(|conn| {
                        create_schema(&conn)?;
                        apply_indexes_to_day_file(&conn, &wanted.attributes)
                    })
                    .map_err(anyhow::Error::from),
            };
            if let Err(error) = result {
                tracing::error!(%day, "index the attributes: {error:#}");
            }
        }
        self.applied_indexes = Some(wanted);
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
                "a metric has more than {MAX_SERIES_PER_METRIC} series a day, and the points of \
                 its newer series were rejected"
            );
            self.reported_rejected_points = self.rejected_points;
        }
        let recorded_at = otelo_storage::now_unix_nanos();
        #[expect(clippy::cast_precision_loss, reason = "a count below 2^53")]
        let counters = [
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
            labels: Attributes::new(),
            points: Points::Counter(
                Temporality::Cumulative,
                vec![NumberPoint {
                    recorded_at,
                    value: total as f64,
                }],
            ),
        });
        let otelo = Resource {
            service: "otelo".into(),
            attributes: Attributes::new(),
        };
        let mut part = ResourceDayRecords::default();
        for counter in &counters {
            for row in rows_of_points(&counter.points) {
                part.push_point(counter, row);
            }
        }
        if let Err(error) = self.write_day(Day::of(recorded_at), &[(&otelo, part)]) {
            tracing::error!("write the counters of lost telemetry: {error:#}");
        }
    }

    fn roll_up_next_due_metrics(&mut self) -> RollupProgress {
        let Some(rollups) = &mut self.rollups else {
            return RollupProgress::CaughtUp;
        };
        let oldest_raw_at = self.config.oldest_retained_day(Day::today()).start();
        // The reader opens a span for each file and statement. Here no request is their parent,
        // so each would show as a request of otelo itself, every minute.
        let rolled_up =
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                rollups.roll_up_next_due(otelo_storage::now_unix_nanos(), oldest_raw_at)
            });
        rolled_up.unwrap_or_else(|error| {
            tracing::error!("roll up the metrics: {error:#}");
            RollupProgress::CaughtUp
        })
    }

    fn apply_retention(&mut self) {
        let today = Day::today();
        if let Some(rollups) = &mut self.rollups {
            let retention = self.config.metric_retention(today);
            let deleted = rollups
                .delete_summaries_before(retention.oldest_minute_at, retention.oldest_hour_at);
            if let Err(error) = deleted {
                tracing::error!("delete old metric rollups: {error:#}");
            }
        }
        let oldest = self.config.oldest_retained_day(today);
        // Closing the files of past days frees their memory.
        self.files.retain(|&day, _| day == today);
        match delete_day_files_before(&self.config.dir, oldest) {
            Ok(deleted) => {
                for name in deleted {
                    tracing::info!(file = %name, "deleted telemetry past the retention");
                }
            }
            Err(error) => tracing::error!("delete old telemetry: {error:#}"),
        }
    }
}

fn delete_day_files_before(dir: &Path, oldest: Day) -> io::Result<Vec<String>> {
    let mut deleted = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if Day::from_file_name(&name).is_some_and(|day| day < oldest) {
            fs::remove_file(entry.path())?;
            deleted.push(name);
        }
    }
    deleted.sort();
    Ok(deleted)
}

#[derive(Clone, Copy)]
enum PointRow<'a> {
    Number(&'a NumberPoint),
    Histogram(&'a HistogramPoint),
}

impl PointRow<'_> {
    const fn recorded_at(self) -> i64 {
        match self {
            Self::Number(point) => point.recorded_at,
            Self::Histogram(point) => point.recorded_at,
        }
    }
}

fn rows_of_points(points: &Points) -> Vec<PointRow<'_>> {
    match points {
        Points::Gauge(points) | Points::UpDown(points) | Points::Counter(_, points) => {
            points.iter().map(PointRow::Number).collect()
        }
        Points::Histogram(_, points) => points.iter().map(PointRow::Histogram).collect(),
    }
}

#[derive(Default)]
struct ResourceDayRecords<'a> {
    logs: Vec<&'a Log>,
    spans: Vec<&'a Span>,
    points: Vec<(&'a Metric, Vec<PointRow<'a>>)>,
}

impl<'a> ResourceDayRecords<'a> {
    fn len(&self) -> usize {
        let points: usize = self.points.iter().map(|(_, points)| points.len()).sum();
        self.logs.len() + self.spans.len() + points
    }

    fn push_point(&mut self, metric: &'a Metric, point: PointRow<'a>) {
        match self.points.last_mut() {
            Some((last, points)) if ptr::eq(*last, metric) => points.push(point),
            _ => self.points.push((metric, vec![point])),
        }
    }
}

struct DayFile {
    conn: Connection,
    resource_ids_by_hash: HashMap<i64, i64>,
    series: SeriesCache,
    catalog: CatalogCache,
}

impl DayFile {
    fn open(dir: &Path, day: Day, indexed: &BTreeSet<IndexedAttribute>) -> anyhow::Result<Self> {
        let path = dir.join(day.file_name());
        let conn = Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        create_schema(&conn).with_context(|| format!("create the schema in {}", path.display()))?;
        apply_indexes_to_day_file(&conn, indexed)
            .with_context(|| format!("index the attributes in {}", path.display()))?;
        let catalog = CatalogCache::load(&conn)
            .with_context(|| format!("read the attribute catalog of {}", path.display()))?;
        Ok(Self {
            conn,
            resource_ids_by_hash: HashMap::new(),
            series: SeriesCache::default(),
            catalog,
        })
    }

    fn write(&mut self, parts: &[(&Resource, ResourceDayRecords)]) -> anyhow::Result<u64> {
        let tx = self.conn.transaction()?;
        let mut rejected_points = 0;
        let mut delta = CatalogDelta::default();
        let catalog = &mut self.catalog;
        for (resource, part) in parts {
            let stored_resource = find_or_insert_resource_id(
                &tx,
                &mut self.resource_ids_by_hash,
                &resource.service,
                &resource.attributes.to_json(),
            )?;
            if matches!(stored_resource, StoredResource::Inserted(_)) {
                catalog.count_attributes(&mut delta, KeyGroup::Resource, &resource.attributes);
            }
            let resource_id = stored_resource.id();
            let mut insert = tx.prepare_cached(
                "INSERT INTO logs (ts, resource_id, severity, body, trace_id, span_id, attributes, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for log in &part.logs {
                catalog.count_attributes(&mut delta, KeyGroup::Logs, &log.attributes);
                insert.execute(params![
                    log.logged_at,
                    resource_id,
                    log.severity.number(),
                    log.body,
                    log.trace_id.map(|id| id.0),
                    log.span_id.map(|id| id.0),
                    log.attributes.to_json(),
                    log.source,
                ])?;
            }
            let mut insert = tx.prepare_cached(
                "INSERT INTO spans (trace_id, span_id, parent_span_id, resource_id, name, kind,
                                    start_ts, duration_ns, status, attributes, events)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for span in &part.spans {
                catalog.count_attributes(&mut delta, KeyGroup::Spans, &span.attributes);
                catalog.count_value(
                    &mut delta,
                    KeyGroup::SpanNames,
                    "name",
                    &AttributeValue::String(span.name.clone()),
                );
                insert.execute(params![
                    span.trace_id.0,
                    span.span_id.0,
                    span.parent_span_id.map(|id| id.0),
                    resource_id,
                    span.name,
                    span.kind.number(),
                    span.started_at,
                    span.duration_ns,
                    span.status.number(),
                    span.attributes.to_json(),
                    serde_json::to_string(&span.events)?,
                ])?;
            }
            rejected_points += write_points_and_count_rejected(
                &tx,
                &mut self.series,
                resource_id,
                &part.points,
                |labels| {
                    catalog.count_attributes(&mut delta, KeyGroup::Metrics, labels);
                },
            )?;
        }
        catalog.write_delta(&tx, delta)?;
        tx.commit()?;
        Ok(rejected_points)
    }
}

fn write_points_and_count_rejected(
    tx: &Transaction,
    cache: &mut SeriesCache,
    resource_id: i64,
    points_by_metric: &[(&Metric, Vec<PointRow>)],
    mut on_new_series: impl FnMut(&Attributes),
) -> anyhow::Result<u64> {
    let mut rejected_points = 0;
    let mut insert = tx.prepare_cached(
        "INSERT INTO points (series_id, ts, value, histogram) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (series_id, ts) DO UPDATE
         SET value = excluded.value, histogram = excluded.histogram",
    )?;
    for (metric, points) in points_by_metric {
        let series = find_or_insert_series_id(
            tx,
            cache,
            resource_id,
            &SeriesIdentity {
                name: &metric.name,
                kind: metric.points.kind(),
                unit: &metric.unit,
                labels_json: &metric.labels.to_json(),
            },
        )?;
        let series_id = match series {
            StoredSeries::Found(id) => id,
            StoredSeries::Inserted(id) => {
                on_new_series(&metric.labels);
                id
            }
            StoredSeries::PastTheMostOfItsMetric => {
                rejected_points += points.len() as u64;
                continue;
            }
        };
        for point in points {
            let (value, histogram) = match point {
                PointRow::Number(point) => (point.value, None),
                PointRow::Histogram(point) => match point.histogram.check_buckets() {
                    Ok(()) => (
                        point.histogram.sum.unwrap_or(0.0),
                        Some(serde_json::to_string(&point.histogram)?),
                    ),
                    Err(error) => {
                        tracing::debug!(metric = %metric.name, "skipped a point: {error:#}");
                        continue;
                    }
                },
            };
            insert.execute(params![series_id, point.recorded_at(), value, histogram])?;
        }
    }
    Ok(rejected_points)
}

pub fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
}

fn set_aside_day_files_of_another_schema(dir: &Path) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let is_day_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".sqlite") && Day::from_file_name(name).is_some());
        if is_day_file {
            set_aside_day_file_of_another_schema(&path)
                .with_context(|| format!("set {} aside", path.display()))?;
        }
    }
    Ok(())
}

fn set_aside_day_file_of_another_schema(path: &Path) -> anyhow::Result<()> {
    let conn = Connection::open(path)?;
    let version = match read_stored_schema(&conn)? {
        StoredSchema::NotWritten | StoredSchema::Version(SCHEMA_VERSION) => return Ok(()),
        StoredSchema::Version(version) => version,
    };
    // The file has to hold all its rows before it moves without its write-ahead log.
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    drop(conn);
    let aside = path.with_extension(format!("sqlite.schema-{version}"));
    fs::rename(path, &aside)?;
    // A write-ahead log left behind would be read as the log of the next file of the day.
    for suffix in ["sqlite-wal", "sqlite-shm"] {
        match fs::remove_file(path.with_extension(suffix)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }
    tracing::warn!(
        file = %aside.display(),
        "set aside a day file of schema version {version}; this otelo reads version \
         {SCHEMA_VERSION}"
    );
    Ok(())
}
