use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use std::{fs, io, ptr, thread};

use anyhow::Context;
use otelo_storage::{
    AttributeValue, Attributes, BatchInbox, HistogramPoint, IndexedAttribute, Log, Metric,
    MetricRetention, NumberPoint, Points, Records, Resource, Span, Temporality,
};
use rusqlite::{Connection, Transaction, params};

use crate::catalog::{CatalogCache, CatalogDelta, KeyGroup};
use crate::day::Day;
use crate::indexes::{Indexes, VersionedAttributes, apply_indexes_to_day_file};
use crate::rollup::{
    RollupProgress, Rollups, delete_rollup_files_set_aside_before,
    set_aside_rollup_file_of_another_schema,
};
use crate::series::{
    MAX_SERIES_PER_METRIC, ResourceId, SeriesCache, SeriesIdentity, StoredResource, StoredSeries,
    find_or_insert_resource_id, find_or_insert_series_id,
};
use crate::stored_schema::set_aside_file_of_another_schema;

const DAY_FILE_SCHEMA: &str = include_str!("schema.sql");

// A day file of another version keeps tables this code cannot read or write.
const DAY_FILE_SCHEMA_VERSION: i32 = 2;

const NO_INDEXED_ATTRIBUTES: &BTreeSet<IndexedAttribute> = &BTreeSet::new();

const DEFAULT_RETENTION_DAYS: NonZeroU16 = NonZeroU16::new(7).expect("seven is not zero");
const DEFAULT_MINUTE_ROLLUP_RETENTION_DAYS: NonZeroU16 =
    NonZeroU16::new(14).expect("fourteen is not zero");
const DEFAULT_HOUR_ROLLUP_RETENTION_DAYS: NonZeroU16 =
    NonZeroU16::new(90).expect("ninety is not zero");
const LOSS_REPORT_INTERVAL: Duration = Duration::from_mins(1);
const RETENTION_INTERVAL: Duration = Duration::from_hours(1);
const ROLLUP_INTERVAL: Duration = Duration::from_mins(1);
const MAX_BATCHES_PER_TRANSACTION: usize = 64;
const INDEX_CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct Config {
    pub directory: PathBuf,
    pub retention_days: NonZeroU16,
    pub minute_rollup_retention_days: NonZeroU16,
    pub hour_rollup_retention_days: NonZeroU16,
    pub indexes: Indexes,
}

impl Config {
    #[must_use]
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            retention_days: DEFAULT_RETENTION_DAYS,
            minute_rollup_retention_days: DEFAULT_MINUTE_ROLLUP_RETENTION_DAYS,
            hour_rollup_retention_days: DEFAULT_HOUR_ROLLUP_RETENTION_DAYS,
            indexes: Indexes::new(BTreeSet::new()),
        }
    }

    pub(crate) fn oldest_retained_day(&self, today: Day) -> Day {
        today.add_days(1 - i64::from(self.retention_days.get()))
    }

    pub(crate) fn metric_retention(&self, today: Day) -> MetricRetention {
        let oldest_kept_at = |retention_days: NonZeroU16| {
            today
                .add_days(1 - i64::from(retention_days.get()))
                .start_at()
        };
        MetricRetention {
            oldest_raw_at: oldest_kept_at(self.retention_days),
            oldest_minute_at: oldest_kept_at(self.minute_rollup_retention_days),
            oldest_hour_at: oldest_kept_at(self.hour_rollup_retention_days),
        }
    }
}

pub struct Writer {
    thread: JoinHandle<()>,
}

impl Writer {
    pub fn spawn(config: Config, inbox: BatchInbox) -> anyhow::Result<Self> {
        fs::create_dir_all(&config.directory).with_context(|| {
            format!(
                "make the telemetry directory {}",
                config.directory.display()
            )
        })?;
        // No reader has a file open yet, so a file can move.
        set_aside_day_files_of_another_schema(&config.directory)?;
        set_aside_rollup_file_of_another_schema(&config.directory)?;
        let thread = thread::Builder::new()
            .name("telemetry-writer".into())
            .spawn(move || WriterState::new(config).write_batches_until_disconnected(&inbox))
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
    day_files: HashMap<Day, DayFile>,
    reported_dropped_batches: u64,
    rejected_points: u64,
    reported_rejected_points: u64,
    applied_indexes: Option<VersionedAttributes>,
    // A rollup file that does not open must not stop the writer.
    rollups: Option<Rollups>,
}

impl WriterState {
    fn new(config: Config) -> Self {
        let rollups = Rollups::open(&config.directory)
            .inspect_err(|error| tracing::error!("no metric rollups: {error:#}"))
            .ok();
        Self {
            rollups,
            config,
            day_files: HashMap::new(),
            reported_dropped_batches: 0,
            rejected_points: 0,
            reported_rejected_points: 0,
            applied_indexes: None,
        }
    }

    fn write_batches_until_disconnected(mut self, inbox: &BatchInbox) {
        self.apply_retention();
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
            if now >= next_retention_at {
                self.apply_retention();
                next_retention_at = now + RETENTION_INTERVAL;
            }
            if now >= next_rollup_at {
                // A daemon that was down has hours to roll up, and takes batches in between.
                next_rollup_at = match self.roll_up_next_due_metrics() {
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
        let retained_days = self.config.oldest_retained_day(today)..=today.add_days(1);
        let mut records_by_day_and_resource: BTreeMap<(Day, usize), ResourceDayRecords<'a>> =
            BTreeMap::new();
        let mut resources = Vec::new();
        for (resource_index, resource_records) in records.enumerate() {
            resources.push(&resource_records.resource);
            for log in &resource_records.logs {
                let day_and_resource = (Day::from_unix_nanos(log.logged_at), resource_index);
                records_by_day_and_resource
                    .entry(day_and_resource)
                    .or_default()
                    .logs
                    .push(log);
            }
            for span in &resource_records.spans {
                let day_and_resource = (Day::from_unix_nanos(span.started_at), resource_index);
                records_by_day_and_resource
                    .entry(day_and_resource)
                    .or_default()
                    .spans
                    .push(span);
            }
            for metric in &resource_records.metrics {
                for point_row in rows_of_points(&metric.points) {
                    let day_and_resource = (
                        Day::from_unix_nanos(point_row.recorded_at()),
                        resource_index,
                    );
                    records_by_day_and_resource
                        .entry(day_and_resource)
                        .or_default()
                        .push_point(metric, point_row);
                }
            }
        }
        let mut resource_records_by_day: BTreeMap<Day, Vec<(&Resource, ResourceDayRecords)>> =
            BTreeMap::new();
        let mut skipped_records = 0;
        for ((day, resource_index), records) in records_by_day_and_resource {
            if retained_days.contains(&day) {
                resource_records_by_day
                    .entry(day)
                    .or_default()
                    .push((resources[resource_index], records));
            } else {
                skipped_records += records.len();
            }
        }
        if skipped_records > 0 {
            tracing::debug!(
                skipped = skipped_records,
                "skipped records outside the retention"
            );
        }
        for (day, resource_records) in resource_records_by_day {
            match self.write_day_records(day, &resource_records) {
                Ok(rejected_points) => self.rejected_points += rejected_points,
                Err(error) => tracing::error!(%day, "write telemetry: {error:#}"),
            }
        }
    }

    fn write_day_records(
        &mut self,
        day: Day,
        resource_records: &[(&Resource, ResourceDayRecords)],
    ) -> anyhow::Result<u64> {
        let day_file = match self.day_files.entry(day) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(DayFile::open(
                &self.config.directory,
                day,
                self.applied_indexes
                    .as_ref()
                    .map_or(NO_INDEXED_ATTRIBUTES, |applied| &applied.attributes),
            )?),
        };
        let result = day_file.write_records(resource_records);
        if result.is_err() {
            day_file.reload_caches_after_rollback();
        }
        result
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
        let days_with_files = match fs::read_dir(&self.config.directory) {
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
        for day in days_with_files {
            let result = match self.day_files.get(&day) {
                Some(day_file) => {
                    apply_indexes_to_day_file(&day_file.connection, &wanted_indexes.attributes)
                        .map_err(anyhow::Error::from)
                }
                // A file from an older otelo may lack the newer tables.
                None => Connection::open(self.config.directory.join(day.file_name()))
                    .and_then(|connection| {
                        create_day_file_schema(&connection)?;
                        apply_indexes_to_day_file(&connection, &wanted_indexes.attributes)
                    })
                    .map_err(anyhow::Error::from),
            };
            if let Err(error) = result {
                tracing::error!(%day, "index the attributes: {error:#}");
            }
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
                "a metric has more than {MAX_SERIES_PER_METRIC} series a day, and the points of \
                 its newer series were rejected"
            );
            self.reported_rejected_points = self.rejected_points;
        }
        let recorded_at = otelo_storage::now_unix_nanos();
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
            labels: Attributes::new(),
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
        let mut records = ResourceDayRecords::default();
        for loss_counter in &loss_counters {
            for point_row in rows_of_points(&loss_counter.points) {
                records.push_point(loss_counter, point_row);
            }
        }
        if let Err(error) = self.write_day_records(
            Day::from_unix_nanos(recorded_at),
            &[(&otelo_resource, records)],
        ) {
            tracing::error!("write the counters of lost telemetry: {error:#}");
        }
    }

    fn roll_up_next_due_metrics(&mut self) -> RollupProgress {
        let Some(rollups) = &mut self.rollups else {
            return RollupProgress::CaughtUp;
        };
        let oldest_raw_at = self.config.oldest_retained_day(Day::today()).start_at();
        // The reader opens a span for each file and statement. Here no request is their parent,
        // so each would show as a request of otelo itself, every minute.
        let progress =
            tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
                rollups.roll_up_next_due(otelo_storage::now_unix_nanos(), oldest_raw_at)
            });
        progress.unwrap_or_else(|error| {
            tracing::error!("roll up the metrics: {error:#}");
            RollupProgress::CaughtUp
        })
    }

    fn apply_retention(&mut self) {
        let today = Day::today();
        let retention = self.config.metric_retention(today);
        if let Some(rollups) = &mut self.rollups {
            let deleted = rollups
                .delete_summaries_before(retention.oldest_minute_at, retention.oldest_hour_at);
            if let Err(error) = deleted {
                tracing::error!("delete old metric rollups: {error:#}");
            }
        }
        match delete_rollup_files_set_aside_before(&self.config.directory, retention.oldest_hour_at)
        {
            Ok(deleted_file_names) => {
                for name in deleted_file_names {
                    tracing::info!(file = %name, "deleted metric rollups past the retention");
                }
            }
            Err(error) => tracing::error!("delete old metric rollups set aside: {error:#}"),
        }
        let oldest_retained_day = self.config.oldest_retained_day(today);
        // Closing the files of past days frees their memory.
        self.day_files.retain(|&day, _| day == today);
        match delete_day_files_before(&self.config.directory, oldest_retained_day) {
            Ok(deleted_file_names) => {
                for name in deleted_file_names {
                    tracing::info!(file = %name, "deleted telemetry past the retention");
                }
            }
            Err(error) => tracing::error!("delete old telemetry: {error:#}"),
        }
    }
}

fn delete_day_files_before(directory: &Path, oldest_retained_day: Day) -> io::Result<Vec<String>> {
    let mut deleted_file_names = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if Day::from_file_name(&name).is_some_and(|day| day < oldest_retained_day) {
            fs::remove_file(entry.path())?;
            deleted_file_names.push(name);
        }
    }
    deleted_file_names.sort();
    Ok(deleted_file_names)
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
    point_rows_by_metric: Vec<(&'a Metric, Vec<PointRow<'a>>)>,
}

impl<'a> ResourceDayRecords<'a> {
    fn len(&self) -> usize {
        let point_count: usize = self
            .point_rows_by_metric
            .iter()
            .map(|(_, point_rows)| point_rows.len())
            .sum();
        self.logs.len() + self.spans.len() + point_count
    }

    fn push_point(&mut self, metric: &'a Metric, point_row: PointRow<'a>) {
        match self.point_rows_by_metric.last_mut() {
            Some((last_metric, point_rows)) if ptr::eq(*last_metric, metric) => {
                point_rows.push(point_row);
            }
            _ => self.point_rows_by_metric.push((metric, vec![point_row])),
        }
    }
}

struct DayFile {
    connection: Connection,
    resource_ids_by_hash: HashMap<i64, ResourceId>,
    series_cache: SeriesCache,
    catalog: CatalogCache,
}

impl DayFile {
    fn open(
        directory: &Path,
        day: Day,
        indexed_attributes: &BTreeSet<IndexedAttribute>,
    ) -> anyhow::Result<Self> {
        let path = directory.join(day.file_name());
        let connection =
            Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        create_day_file_schema(&connection)
            .with_context(|| format!("create the schema in {}", path.display()))?;
        apply_indexes_to_day_file(&connection, indexed_attributes)
            .with_context(|| format!("index the attributes in {}", path.display()))?;
        let catalog = CatalogCache::load(&connection)
            .with_context(|| format!("read the attribute catalog of {}", path.display()))?;
        Ok(Self {
            connection,
            resource_ids_by_hash: HashMap::new(),
            series_cache: SeriesCache::default(),
            catalog,
        })
    }

    fn reload_caches_after_rollback(&mut self) {
        self.resource_ids_by_hash.clear();
        self.series_cache = SeriesCache::default();
        self.catalog = CatalogCache::load(&self.connection).unwrap_or_default();
    }

    fn write_records(
        &mut self,
        resource_records: &[(&Resource, ResourceDayRecords)],
    ) -> anyhow::Result<u64> {
        let transaction = self.connection.transaction()?;
        let mut rejected_points = 0;
        let mut catalog_delta = CatalogDelta::default();
        let catalog = &mut self.catalog;
        for (resource, records) in resource_records {
            let stored_resource = find_or_insert_resource_id(
                &transaction,
                &mut self.resource_ids_by_hash,
                &resource.service,
                &resource.attributes.to_json(),
            )?;
            if matches!(stored_resource, StoredResource::Inserted(_)) {
                catalog.count_attributes(
                    &mut catalog_delta,
                    KeyGroup::Resource,
                    &resource.attributes,
                );
            }
            let resource_id = stored_resource.id();
            let mut insert_log = transaction.prepare_cached(
                "INSERT INTO logs (logged_at, resource_id, severity, body, trace_id, span_id,
                                   attributes, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for log in &records.logs {
                catalog.count_attributes(&mut catalog_delta, KeyGroup::Logs, &log.attributes);
                insert_log.execute(params![
                    log.logged_at,
                    resource_id,
                    log.severity.number(),
                    log.body,
                    log.trace_context.trace_id().map(|trace_id| trace_id.0),
                    log.trace_context.span_id().map(|span_id| span_id.0),
                    log.attributes.to_json(),
                    log.source.name(),
                ])?;
            }
            let mut insert_span = transaction.prepare_cached(
                "INSERT INTO spans (trace_id, span_id, parent_span_id, resource_id, name, kind,
                                    started_at, duration_ns, status, attributes, events)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for span in &records.spans {
                catalog.count_attributes(&mut catalog_delta, KeyGroup::Spans, &span.attributes);
                catalog.count_value(
                    &mut catalog_delta,
                    KeyGroup::SpanNames,
                    "name",
                    &AttributeValue::String(span.name.clone()),
                );
                insert_span.execute(params![
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
                &transaction,
                &mut self.series_cache,
                resource_id,
                &records.point_rows_by_metric,
                |labels| {
                    catalog.count_attributes(&mut catalog_delta, KeyGroup::Metrics, labels);
                },
            )?;
        }
        catalog.write_delta(&transaction, catalog_delta)?;
        transaction.commit()?;
        Ok(rejected_points)
    }
}

fn write_points_and_count_rejected(
    transaction: &Transaction,
    series_cache: &mut SeriesCache,
    resource_id: ResourceId,
    point_rows_by_metric: &[(&Metric, Vec<PointRow>)],
    mut on_new_series: impl FnMut(&Attributes),
) -> anyhow::Result<u64> {
    let mut rejected_points = 0;
    let mut insert_point = transaction.prepare_cached(
        "INSERT INTO points (series_id, recorded_at, value, histogram) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (series_id, recorded_at) DO UPDATE
         SET value = excluded.value, histogram = excluded.histogram",
    )?;
    for (metric, point_rows) in point_rows_by_metric {
        let stored_series = find_or_insert_series_id(
            transaction,
            series_cache,
            resource_id,
            &SeriesIdentity {
                name: &metric.name,
                kind: metric.points.kind(),
                unit: &metric.unit,
                labels_json: &metric.labels.to_json(),
            },
        )?;
        let series_id = match stored_series {
            StoredSeries::Found(series_id) => series_id,
            StoredSeries::Inserted(series_id) => {
                on_new_series(&metric.labels);
                series_id
            }
            StoredSeries::PastSeriesLimit => {
                rejected_points += point_rows.len() as u64;
                continue;
            }
        };
        for point_row in point_rows {
            let (value, histogram_json) = match point_row {
                PointRow::Number(point) => (point.value, None),
                PointRow::Histogram(point) => (
                    point.histogram.sum.unwrap_or(0.0),
                    Some(serde_json::to_string(&point.histogram)?),
                ),
            };
            insert_point.execute(params![
                series_id,
                point_row.recorded_at(),
                value,
                histogram_json
            ])?;
        }
    }
    Ok(rejected_points)
}

pub fn create_day_file_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(DAY_FILE_SCHEMA)?;
    connection.pragma_update(None, "user_version", DAY_FILE_SCHEMA_VERSION)
}

fn set_aside_day_files_of_another_schema(directory: &Path) -> anyhow::Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let is_day_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".sqlite") && Day::from_file_name(name).is_some());
        if is_day_file {
            set_aside_file_of_another_schema(&path, DAY_FILE_SCHEMA_VERSION)
                .with_context(|| format!("set {} aside", path.display()))?;
        }
    }
    Ok(())
}
