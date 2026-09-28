use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use std::{fs, io, ptr, thread};

use anyhow::Context;
use rusqlite::{Connection, Transaction, params};
use serde_json::Map;
use twox_hash::XxHash3_64;

use crate::catalog::{Catalog, Delta, Group};
use crate::day::{self, Day};
use crate::indexes::{self, IndexedKey, Indexes};
use crate::{Batch, Log, Metric, MetricKind, Point, Records, Resource, Span};

pub const SCHEMA: &str = include_str!("schema.sql");

/// How often the writer records the drop counter as a metric.
const REPORT_EVERY: Duration = Duration::from_mins(1);
/// How often retention runs after the one at start.
const RETAIN_EVERY: Duration = Duration::from_hours(1);
/// The most batches the writer takes into one transaction.
const MAX_ROUND: usize = 64;
/// How long a change to the indexed attributes waits for the writer.
const CHECK_INDEXES_EVERY: Duration = Duration::from_secs(1);

pub struct Config {
    /// The directory of the day files, made when it is missing.
    pub dir: PathBuf,
    /// How many days of files to keep, today included.
    pub retention_days: u16,
    /// The attributes to index in every day file.
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
}

/// Makes the channel from the sources to the writer. It holds `capacity`
/// batches.
#[must_use]
pub fn channel(capacity: usize) -> (Sender, Inbox) {
    let (tx, rx) = sync_channel(capacity);
    let dropped = Arc::new(AtomicU64::new(0));
    (
        Sender {
            tx,
            dropped: Arc::clone(&dropped),
        },
        Inbox { rx, dropped },
    )
}

/// The end of the channel that a source holds.
#[derive(Clone)]
pub struct Sender {
    tx: SyncSender<Batch>,
    dropped: Arc<AtomicU64>,
}

impl Sender {
    /// Queues `batch` for the writer. When the channel is full, drops the
    /// batch and counts the drop, so a burst never waits or grows memory.
    pub fn send(&self, batch: Batch) {
        match self.tx.try_send(batch) {
            Ok(()) => {}
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// How many batches the channel has dropped.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// The end of the channel that the writer reads.
pub struct Inbox {
    rx: Receiver<Batch>,
    dropped: Arc<AtomicU64>,
}

/// The thread that owns every write to the day files and deletes the old ones.
pub struct Writer {
    thread: JoinHandle<()>,
}

impl Writer {
    /// Makes the directory, runs retention, and starts the writer thread.
    ///
    /// # Errors
    ///
    /// When the directory cannot be made or the thread cannot start.
    pub fn spawn(config: Config, inbox: Inbox) -> anyhow::Result<Self> {
        fs::create_dir_all(&config.dir)
            .with_context(|| format!("make the telemetry directory {}", config.dir.display()))?;
        let thread = thread::Builder::new()
            .name("telemetry-writer".into())
            .spawn(move || State::new(config, inbox.dropped).run(&inbox.rx))
            .context("start the telemetry writer")?;
        Ok(Self { thread })
    }

    /// Waits until every [`Sender`] is dropped and the writer has written
    /// what they sent.
    ///
    /// # Errors
    ///
    /// When the writer thread panicked.
    pub fn join(self) -> anyhow::Result<()> {
        self.thread
            .join()
            .map_err(|_| anyhow::anyhow!("the telemetry writer panicked"))
    }
}

struct State {
    config: Config,
    files: HashMap<Day, DayFile>,
    dropped: Arc<AtomicU64>,
    reported: u64,
    /// The indexed attributes the day files have, and the version of the set.
    indexed: (Option<u64>, BTreeSet<IndexedKey>),
}

impl State {
    fn new(config: Config, dropped: Arc<AtomicU64>) -> Self {
        Self {
            config,
            files: HashMap::new(),
            dropped,
            reported: 0,
            indexed: (None, BTreeSet::new()),
        }
    }

    fn run(mut self, inbox: &Receiver<Batch>) {
        self.retain();
        let mut next_report = Instant::now() + REPORT_EVERY;
        let mut next_retain = Instant::now() + RETAIN_EVERY;
        loop {
            self.apply_indexes();
            let wait = next_report
                .min(next_retain)
                .saturating_duration_since(Instant::now())
                .min(CHECK_INDEXES_EVERY);
            match inbox.recv_timeout(wait) {
                Ok(batch) => {
                    let mut batches = vec![batch];
                    batches.extend(inbox.try_iter().take(MAX_ROUND - 1));
                    self.write(batches.iter().flatten());
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let now = Instant::now();
            if now >= next_report {
                self.report_drops();
                next_report = now + REPORT_EVERY;
            }
            if now >= next_retain {
                self.retain();
                next_retain = now + RETAIN_EVERY;
            }
        }
        self.report_drops();
    }

    /// Writes one transaction per day file. A record outside the retention,
    /// or more than a day ahead, is skipped, because retention would delete
    /// its file or the file would outlive it.
    fn write<'a>(&mut self, records: impl Iterator<Item = &'a Records>) {
        let today = Day::today();
        let window = self.oldest(today)..=today.plus(1);
        let mut parts: BTreeMap<(Day, usize), Part<'a>> = BTreeMap::new();
        let mut resources = Vec::new();
        for (index, records) in records.enumerate() {
            resources.push(&records.resource);
            for log in &records.logs {
                let key = (Day::of(log.ts), index);
                parts.entry(key).or_default().logs.push(log);
            }
            for span in &records.spans {
                let key = (Day::of(span.start_ts), index);
                parts.entry(key).or_default().spans.push(span);
            }
            for metric in &records.metrics {
                for point in &metric.points {
                    let key = (Day::of(point.ts), index);
                    parts.entry(key).or_default().push_point(metric, point);
                }
            }
        }
        let mut days: BTreeMap<Day, Vec<(&Resource, Part)>> = BTreeMap::new();
        let mut skipped = 0;
        for ((day, index), part) in parts {
            if window.contains(&day) {
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

    fn write_day(&mut self, day: Day, parts: &[(&Resource, Part)]) -> anyhow::Result<()> {
        let file = match self.files.entry(day) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                entry.insert(DayFile::open(&self.config.dir, day, &self.indexed.1)?)
            }
        };
        let result = file.write(parts);
        if result.is_err() {
            // The rollback took back what the caches learned.
            file.resources.clear();
            file.series.clear();
            file.catalog = Catalog::load(&file.conn).unwrap_or_default();
        }
        result
    }

    /// Brings the indexes of every day file in line with the configured set,
    /// when the set changed.
    fn apply_indexes(&mut self) {
        let (version, keys) = self.config.indexes.snapshot();
        if self.indexed.0 == Some(version) {
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
                Some(file) => indexes::apply(&file.conn, &keys).map_err(anyhow::Error::from),
                // A file from an older siner may lack the newer tables.
                None => Connection::open(self.config.dir.join(day.file_name()))
                    .and_then(|conn| {
                        conn.execute_batch(SCHEMA)?;
                        indexes::apply(&conn, &keys)
                    })
                    .map_err(anyhow::Error::from),
            };
            if let Err(error) = result {
                tracing::error!(%day, "index the attributes: {error:#}");
            }
        }
        self.indexed = (Some(version), keys);
    }

    /// Records the drop counter as a cumulative sum in today's file.
    fn report_drops(&mut self) {
        let dropped = self.dropped.load(Ordering::Relaxed);
        if dropped > self.reported {
            tracing::warn!(
                dropped = dropped - self.reported,
                "the telemetry channel was full and dropped batches"
            );
            self.reported = dropped;
        }
        #[expect(clippy::cast_precision_loss, reason = "a count below 2^53")]
        let point = Point {
            ts: day::now(),
            value: dropped as f64,
            histogram: None,
        };
        let metric = Metric {
            name: "siner.telemetry.dropped_batches".into(),
            kind: MetricKind::Sum,
            unit: "{batch}".into(),
            labels: Map::new(),
            points: Vec::new(),
        };
        let siner = Resource {
            service: "siner".into(),
            attributes: Map::new(),
        };
        let mut part = Part::default();
        part.push_point(&metric, &point);
        if let Err(error) = self.write_day(Day::of(point.ts), &[(&siner, part)]) {
            tracing::error!("write the drop counter: {error:#}");
        }
    }

    /// Deletes the files of the days before the retention, and closes every
    /// file but today's, so a day that gets no more records frees its memory.
    fn retain(&mut self) {
        let today = Day::today();
        let oldest = self.oldest(today);
        self.files.retain(|&day, _| day == today);
        match delete_before(&self.config.dir, oldest) {
            Ok(deleted) => {
                for name in deleted {
                    tracing::info!(file = %name, "deleted telemetry past the retention");
                }
            }
            Err(error) => tracing::error!("delete old telemetry: {error:#}"),
        }
    }

    fn oldest(&self, today: Day) -> Day {
        today.plus(1 - i64::from(self.config.retention_days.max(1)))
    }
}

/// Deletes the day files before `oldest` and returns their names. Files that
/// are not day files stay.
fn delete_before(dir: &Path, oldest: Day) -> io::Result<Vec<String>> {
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

/// The records of one resource that fall on one day.
#[derive(Default)]
struct Part<'a> {
    logs: Vec<&'a Log>,
    spans: Vec<&'a Span>,
    points: Vec<(&'a Metric, Vec<&'a Point>)>,
}

impl<'a> Part<'a> {
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
    /// Resource hash to id.
    resources: HashMap<i64, i64>,
    /// Series hash to id.
    series: HashMap<i64, i64>,
    catalog: Catalog,
}

impl DayFile {
    fn open(dir: &Path, day: Day, indexed: &BTreeSet<IndexedKey>) -> anyhow::Result<Self> {
        let path = dir.join(day.file_name());
        let conn = Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch(SCHEMA)
            .with_context(|| format!("create the schema in {}", path.display()))?;
        indexes::apply(&conn, indexed)
            .with_context(|| format!("index the attributes in {}", path.display()))?;
        let catalog = Catalog::load(&conn)
            .with_context(|| format!("read the attribute catalog of {}", path.display()))?;
        Ok(Self {
            conn,
            resources: HashMap::new(),
            series: HashMap::new(),
            catalog,
        })
    }

    fn write(&mut self, parts: &[(&Resource, Part)]) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        let mut delta = Delta::default();
        let catalog = &mut self.catalog;
        for (resource, part) in parts {
            let resource_id = resource_id(&tx, &mut self.resources, resource, |attributes| {
                catalog.record(&mut delta, Group::Resource, attributes);
            })?;
            let mut insert = tx.prepare_cached(
                "INSERT INTO logs (ts, resource_id, severity, body, trace_id, span_id, attributes, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for log in &part.logs {
                catalog.record(&mut delta, Group::Logs, &log.attributes);
                insert.execute(params![
                    log.ts,
                    resource_id,
                    log.severity,
                    log.body,
                    log.trace_id,
                    log.span_id,
                    serde_json::to_string(&log.attributes)?,
                    log.source,
                ])?;
            }
            let mut insert = tx.prepare_cached(
                "INSERT INTO spans (trace_id, span_id, parent_span_id, resource_id, name, kind,
                                    start_ts, duration_ns, status, attributes, events)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for span in &part.spans {
                catalog.record(&mut delta, Group::Spans, &span.attributes);
                catalog.value(
                    &mut delta,
                    Group::SpanNames,
                    "name",
                    &serde_json::Value::String(span.name.clone()),
                );
                insert.execute(params![
                    span.trace_id,
                    span.span_id,
                    span.parent_span_id,
                    resource_id,
                    span.name,
                    span.kind,
                    span.start_ts,
                    span.duration_ns,
                    span.status,
                    serde_json::to_string(&span.attributes)?,
                    serde_json::to_string(&span.events)?,
                ])?;
            }
            let mut insert = tx.prepare_cached(
                "INSERT INTO points (series_id, ts, value, histogram) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (metric, points) in &part.points {
                let series_id = series_id(&tx, &mut self.series, resource_id, metric, |labels| {
                    catalog.record(&mut delta, Group::Metrics, labels);
                })?;
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
                    insert.execute(params![series_id, point.ts, point.value, histogram])?;
                }
            }
        }
        catalog.flush(&tx, delta)?;
        tx.commit()?;
        Ok(())
    }
}

/// The id of `resource` in the day file. Calls `new` with the attributes when
/// the resource is new to the file.
fn resource_id(
    tx: &Transaction,
    cache: &mut HashMap<i64, i64>,
    resource: &Resource,
    new: impl FnOnce(&Map<String, serde_json::Value>),
) -> anyhow::Result<i64> {
    let attributes = serde_json::to_string(&resource.attributes)?;
    let hash = hash(&[&resource.service, &attributes]);
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
        new(&resource.attributes);
    }
    let id = tx
        .prepare_cached("SELECT id FROM resources WHERE hash = ?1")?
        .query_row([hash], |row| row.get(0))?;
    cache.insert(hash, id);
    Ok(id)
}

/// The id of the series of `metric` in the day file. Calls `new` with the
/// labels when the series is new to the file.
fn series_id(
    tx: &Transaction,
    cache: &mut HashMap<i64, i64>,
    resource_id: i64,
    metric: &Metric,
    new: impl FnOnce(&Map<String, serde_json::Value>),
) -> anyhow::Result<i64> {
    let labels = serde_json::to_string(&metric.labels)?;
    let kind = metric.kind.as_str();
    let hash = hash(&[
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
        new(&metric.labels);
    }
    let id = tx
        .prepare_cached("SELECT id FROM series WHERE hash = ?1")?
        .query_row([hash], |row| row.get(0))?;
    cache.insert(hash, id);
    Ok(id)
}

/// The xxh3 of the fields, each after its length, so that no two lists of
/// fields hash the same bytes. Serde writes JSON with sorted keys, so equal
/// attributes hash equal.
pub fn hash(fields: &[&str]) -> i64 {
    let mut bytes = Vec::new();
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u64).to_le_bytes());
        bytes.extend_from_slice(field.as_bytes());
    }
    XxHash3_64::oneshot(&bytes).cast_signed()
}
