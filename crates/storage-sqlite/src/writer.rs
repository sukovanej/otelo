use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use std::{fs, io, ptr, thread};

use anyhow::Context;
use otelo_storage::{
    AttributeValue, Attributes, Inbox, IndexedAttribute, Log, Metric, MetricKind, Point, Records,
    Resource, Span,
};
use rusqlite::{Connection, Transaction, params};
use twox_hash::XxHash3_64;

use crate::catalog::{CatalogCache, CatalogDelta, KeyGroup};
use crate::day::Day;
use crate::indexes::{Indexes, VersionedAttributes, apply_indexes_to_day_file};

pub const SCHEMA: &str = include_str!("schema.sql");

const NO_ATTRIBUTES: &BTreeSet<IndexedAttribute> = &BTreeSet::new();

const DROP_REPORT_INTERVAL: Duration = Duration::from_mins(1);
const RETENTION_INTERVAL: Duration = Duration::from_hours(1);
const MAX_BATCHES_PER_TRANSACTION: usize = 64;
const INDEX_CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct Config {
    pub dir: PathBuf,
    pub retention_days: u16,
    pub indexes: Indexes,
}

impl Config {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            retention_days: 7,
            indexes: Indexes::new(BTreeSet::new()),
        }
    }

    pub(crate) fn oldest_retained_day(&self, today: Day) -> Day {
        today.plus(1 - i64::from(self.retention_days.max(1)))
    }
}

pub struct Writer {
    thread: JoinHandle<()>,
}

impl Writer {
    pub fn spawn(config: Config, inbox: Inbox) -> anyhow::Result<Self> {
        fs::create_dir_all(&config.dir)
            .with_context(|| format!("make the telemetry directory {}", config.dir.display()))?;
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
    applied_indexes: Option<VersionedAttributes>,
}

impl State {
    fn new(config: Config) -> Self {
        Self {
            config,
            files: HashMap::new(),
            reported_dropped_batches: 0,
            applied_indexes: None,
        }
    }

    fn run(mut self, inbox: &Inbox) {
        self.apply_retention();
        let mut next_report = Instant::now() + DROP_REPORT_INTERVAL;
        let mut next_retain = Instant::now() + RETENTION_INTERVAL;
        loop {
            self.apply_index_changes();
            let wait = next_report
                .min(next_retain)
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
                self.report_dropped_batches(inbox.dropped_batches());
                next_report = now + DROP_REPORT_INTERVAL;
            }
            if now >= next_retain {
                self.apply_retention();
                next_retain = now + RETENTION_INTERVAL;
            }
        }
        self.report_dropped_batches(inbox.dropped_batches());
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
                for point in &metric.points {
                    let key = (Day::of(point.recorded_at), index);
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
            if let Err(error) = self.write_day(day, &parts) {
                tracing::error!(%day, "write telemetry: {error:#}");
            }
        }
    }

    fn write_day(
        &mut self,
        day: Day,
        parts: &[(&Resource, ResourceDayRecords)],
    ) -> anyhow::Result<()> {
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
            file.series_ids_by_hash.clear();
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
                        conn.execute_batch(SCHEMA)?;
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

    fn report_dropped_batches(&mut self, dropped_batches: u64) {
        if dropped_batches > self.reported_dropped_batches {
            tracing::warn!(
                dropped = dropped_batches - self.reported_dropped_batches,
                "the telemetry channel was full and dropped batches"
            );
            self.reported_dropped_batches = dropped_batches;
        }
        #[expect(clippy::cast_precision_loss, reason = "a count below 2^53")]
        let point = Point {
            recorded_at: otelo_storage::now_unix_nanos(),
            value: dropped_batches as f64,
            histogram: None,
        };
        let metric = Metric {
            name: "otelo.telemetry.dropped_batches".into(),
            kind: MetricKind::Sum,
            unit: "{batch}".into(),
            labels: Attributes::new(),
            points: Vec::new(),
        };
        let otelo = Resource {
            service: "otelo".into(),
            attributes: Attributes::new(),
        };
        let mut part = ResourceDayRecords::default();
        part.push_point(&metric, &point);
        if let Err(error) = self.write_day(Day::of(point.recorded_at), &[(&otelo, part)]) {
            tracing::error!("write the drop counter: {error:#}");
        }
    }

    fn apply_retention(&mut self) {
        let today = Day::today();
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

#[derive(Default)]
struct ResourceDayRecords<'a> {
    logs: Vec<&'a Log>,
    spans: Vec<&'a Span>,
    points: Vec<(&'a Metric, Vec<&'a Point>)>,
}

impl<'a> ResourceDayRecords<'a> {
    fn len(&self) -> usize {
        let points: usize = self.points.iter().map(|(_, points)| points.len()).sum();
        self.logs.len() + self.spans.len() + points
    }

    fn push_point(&mut self, metric: &'a Metric, point: &'a Point) {
        match self.points.last_mut() {
            Some((last, points)) if ptr::eq(*last, metric) => points.push(point),
            _ => self.points.push((metric, vec![point])),
        }
    }
}

struct DayFile {
    conn: Connection,
    resource_ids_by_hash: HashMap<i64, i64>,
    series_ids_by_hash: HashMap<i64, i64>,
    catalog: CatalogCache,
}

impl DayFile {
    fn open(dir: &Path, day: Day, indexed: &BTreeSet<IndexedAttribute>) -> anyhow::Result<Self> {
        let path = dir.join(day.file_name());
        let conn = Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch(SCHEMA)
            .with_context(|| format!("create the schema in {}", path.display()))?;
        apply_indexes_to_day_file(&conn, indexed)
            .with_context(|| format!("index the attributes in {}", path.display()))?;
        let catalog = CatalogCache::load(&conn)
            .with_context(|| format!("read the attribute catalog of {}", path.display()))?;
        Ok(Self {
            conn,
            resource_ids_by_hash: HashMap::new(),
            series_ids_by_hash: HashMap::new(),
            catalog,
        })
    }

    fn write(&mut self, parts: &[(&Resource, ResourceDayRecords)]) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        let mut delta = CatalogDelta::default();
        let catalog = &mut self.catalog;
        for (resource, part) in parts {
            let resource_id = find_or_insert_resource_id(
                &tx,
                &mut self.resource_ids_by_hash,
                resource,
                |attributes| {
                    catalog.count_attributes(&mut delta, KeyGroup::Resource, attributes);
                },
            )?;
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
            let mut insert = tx.prepare_cached(
                "INSERT INTO points (series_id, ts, value, histogram) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (metric, points) in &part.points {
                let series_id = find_or_insert_series_id(
                    &tx,
                    &mut self.series_ids_by_hash,
                    resource_id,
                    metric,
                    |labels| {
                        catalog.count_attributes(&mut delta, KeyGroup::Metrics, labels);
                    },
                )?;
                for point in points {
                    let histogram = match &point.histogram {
                        Some(histogram) => match histogram.check() {
                            Ok(()) => Some(serde_json::to_string(histogram)?),
                            Err(error) => {
                                tracing::debug!(metric = %metric.name, "skipped a point: {error:#}");
                                continue;
                            }
                        },
                        None => None,
                    };
                    insert.execute(params![
                        series_id,
                        point.recorded_at,
                        point.value,
                        histogram
                    ])?;
                }
            }
        }
        catalog.write_delta(&tx, delta)?;
        tx.commit()?;
        Ok(())
    }
}

fn find_or_insert_resource_id(
    tx: &Transaction,
    cache: &mut HashMap<i64, i64>,
    resource: &Resource,
    on_insert: impl FnOnce(&Attributes),
) -> anyhow::Result<i64> {
    // The JSON has sorted keys, so equal attributes hash equal.
    let attributes = resource.attributes.to_json();
    let hash = hash_fields(&[&resource.service, &attributes]);
    if let Some(&id) = cache.get(&hash) {
        return Ok(id);
    }
    let inserted = tx
        .prepare_cached(
            "INSERT INTO resources (hash, service, attributes) VALUES (?1, ?2, ?3)
             ON CONFLICT (hash) DO NOTHING",
        )?
        .execute(params![hash, resource.service, attributes])?;
    if inserted > 0 {
        on_insert(&resource.attributes);
    }
    let id = tx
        .prepare_cached("SELECT id FROM resources WHERE hash = ?1")?
        .query_row([hash], |row| row.get(0))?;
    cache.insert(hash, id);
    Ok(id)
}

fn find_or_insert_series_id(
    tx: &Transaction,
    cache: &mut HashMap<i64, i64>,
    resource_id: i64,
    metric: &Metric,
    on_insert: impl FnOnce(&Attributes),
) -> anyhow::Result<i64> {
    let labels = metric.labels.to_json();
    let kind = metric.kind.as_str();
    let hash = hash_fields(&[
        &resource_id.to_string(),
        &metric.name,
        kind,
        &metric.unit,
        &labels,
    ]);
    if let Some(&id) = cache.get(&hash) {
        return Ok(id);
    }
    let inserted = tx
        .prepare_cached(
            "INSERT INTO series (hash, resource_id, name, kind, unit, labels)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (hash) DO NOTHING",
        )?
        .execute(params![
            hash,
            resource_id,
            metric.name,
            kind,
            metric.unit,
            labels
        ])?;
    if inserted > 0 {
        on_insert(&metric.labels);
    }
    let id = tx
        .prepare_cached("SELECT id FROM series WHERE hash = ?1")?
        .query_row([hash], |row| row.get(0))?;
    cache.insert(hash, id);
    Ok(id)
}

pub fn hash_fields(fields: &[&str]) -> i64 {
    // A length before each field keeps two lists of fields from hashing the same bytes.
    let mut bytes = Vec::new();
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u64).to_le_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    XxHash3_64::oneshot(&bytes).cast_signed()
}
