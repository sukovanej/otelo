use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::Context;
use otelo_indexed_storage::{
    HistogramPoint, IndexedAttribute, Log, Metric, NumberPoint, Points, Resource, Span,
};
use otelo_journal::{Hour, Position};
use otelo_query::Signal;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::catalog::{AttributeOwner, CatalogCache, CatalogDelta};
use crate::day::Day;
use crate::indexes::apply_indexes_to_telemetry_file;
use crate::retention::RetentionStage;
use crate::series::{
    MetricSeriesId, ResourceId, SeriesCache, SeriesIdentity, StoredSeries,
    find_or_insert_resource_id, find_or_insert_series_id,
};
use crate::version::{OtherStorageVersion, STORAGE_VERSION};

pub const TELEMETRY_FILE_NAME: &str = "telemetry.sqlite";

const SCHEMA: &str = include_str!("schema.sql");

// The OS page cache keeps the hot pages, so each connection keeps little of its own.
const PAGE_CACHE_KIB: i64 = 1024;

pub fn keep_small_page_cache(connection: &Connection) -> rusqlite::Result<()> {
    connection.pragma_update(None, "cache_size", -PAGE_CACHE_KIB)
}

#[derive(Clone, Copy)]
pub enum PointRow<'a> {
    Number(&'a NumberPoint),
    Histogram(&'a HistogramPoint),
}

impl PointRow<'_> {
    pub const fn recorded_at(self) -> i64 {
        match self {
            Self::Number(point) => point.recorded_at,
            Self::Histogram(point) => point.recorded_at,
        }
    }
}

pub fn rows_of_points(points: &Points) -> Vec<PointRow<'_>> {
    match points {
        Points::Gauge(points) | Points::UpDown(points) | Points::Counter(_, points) => {
            points.iter().map(PointRow::Number).collect()
        }
        Points::Histogram(_, points) => points.iter().map(PointRow::Histogram).collect(),
    }
}

pub struct ResourceRecords<'a> {
    pub resource: &'a Resource,
    pub logs: Vec<&'a Log>,
    pub spans: Vec<&'a Span>,
    pub point_rows_by_metric: Vec<(&'a Metric, Vec<PointRow<'a>>)>,
}

impl ResourceRecords<'_> {
    pub const fn is_empty(&self) -> bool {
        self.logs.is_empty() && self.spans.is_empty() && self.point_rows_by_metric.is_empty()
    }

    pub fn record_count(&self) -> u64 {
        let point_count: usize = self
            .point_rows_by_metric
            .iter()
            .map(|(_, point_rows)| point_rows.len())
            .sum();
        (self.logs.len() + self.spans.len() + point_count) as u64
    }

    fn days(&self) -> BTreeSet<Day> {
        let log_days = self.logs.iter().map(|log| log.logged_at);
        let span_days = self.spans.iter().map(|span| span.started_at);
        let point_days = self
            .point_rows_by_metric
            .iter()
            .flat_map(|(_, point_rows)| point_rows.iter().map(|point_row| point_row.recorded_at()));
        log_days
            .chain(span_days)
            .chain(point_days)
            .map(Day::from_unix_nanos)
            .collect()
    }
}

pub struct TelemetryFile {
    connection: Connection,
    cached_rows: CachedRows,
    pub(crate) retention_stage: RetentionStage,
}

#[derive(Default)]
struct CachedRows {
    resource_ids_by_identity_hash: HashMap<i64, ResourceId>,
    series_cache: SeriesCache,
    catalog: CatalogCache,
    // A resource and a series outlive a day, and the catalog counts them once on each day.
    counted_resource_days: HashSet<(Day, ResourceId)>,
    counted_series_days: HashSet<(Day, MetricSeriesId)>,
}

impl TelemetryFile {
    pub fn open(directory: &Path) -> anyhow::Result<Self> {
        fs::create_dir_all(directory)
            .with_context(|| format!("make the telemetry directory {}", directory.display()))?;
        let path = directory.join(TELEMETRY_FILE_NAME);
        let connection =
            Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        keep_small_page_cache(&connection)?;
        let found_version: i64 =
            connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let has_tables: bool =
            connection.query_row("SELECT count(*) > 0 FROM sqlite_master", [], |row| {
                row.get(0)
            })?;
        if found_version != STORAGE_VERSION && (has_tables || found_version != 0) {
            return Err(OtherStorageVersion { found_version }.into());
        }
        connection
            .execute_batch(SCHEMA)
            .with_context(|| format!("create the schema in {}", path.display()))?;
        connection.pragma_update(None, "user_version", STORAGE_VERSION)?;
        Ok(Self {
            connection,
            cached_rows: CachedRows::default(),
            retention_stage: RetentionStage::default(),
        })
    }

    pub(crate) const fn connection(&self) -> &Connection {
        &self.connection
    }

    pub(crate) const fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    pub(crate) fn forget_cached_rows(&mut self) {
        self.cached_rows = CachedRows::default();
    }

    pub(crate) fn forget_rows_past_retention(
        &mut self,
        oldest_retained_day: Day,
        deleted_resource_ids: &HashSet<ResourceId>,
        deleted_series_ids: &HashSet<MetricSeriesId>,
    ) {
        let cached_rows = &mut self.cached_rows;
        cached_rows.resource_ids_by_identity_hash.clear();
        cached_rows.series_cache = SeriesCache::default();
        cached_rows.catalog.forget_days_before(oldest_retained_day);
        cached_rows
            .counted_resource_days
            .retain(|(day, resource_id)| {
                *day >= oldest_retained_day && !deleted_resource_ids.contains(resource_id)
            });
        cached_rows.counted_series_days.retain(|(day, series_id)| {
            *day >= oldest_retained_day && !deleted_series_ids.contains(series_id)
        });
    }

    pub(crate) fn apply_indexes(
        &self,
        attributes: &BTreeSet<IndexedAttribute>,
    ) -> rusqlite::Result<()> {
        apply_indexes_to_telemetry_file(&self.connection, attributes)
    }

    pub(crate) fn read_indexed_position(&self, signal: Signal) -> anyhow::Result<Option<Position>> {
        let stored_position = self
            .connection
            .query_row(
                "SELECT segment_hour, byte_offset FROM indexed_journal_positions WHERE signal = ?1",
                [signal.name()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        stored_position
            .map(|(segment_hour, byte_offset)| {
                Ok(Position {
                    segment_hour: Hour::from_hours_since_epoch(segment_hour),
                    byte_offset: u64::try_from(byte_offset)
                        .with_context(|| format!("the byte offset {byte_offset} of {signal}"))?,
                })
            })
            .transpose()
    }

    pub(crate) fn write_indexed_frames(
        &mut self,
        resource_records: &[ResourceRecords],
        signal: Signal,
        position_after: Position,
    ) -> anyhow::Result<u64> {
        let result =
            self.write_indexed_frames_in_transaction(resource_records, signal, position_after);
        if result.is_err() {
            self.forget_cached_rows();
        }
        result
    }

    fn write_indexed_frames_in_transaction(
        &mut self,
        resource_records: &[ResourceRecords],
        signal: Signal,
        position_after: Position,
    ) -> anyhow::Result<u64> {
        let transaction = self.connection.transaction()?;
        let rejected_points = self
            .cached_rows
            .write_records(&transaction, resource_records)?;
        transaction.execute(
            "INSERT INTO indexed_journal_positions (signal, segment_hour, byte_offset)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (signal) DO UPDATE
             SET segment_hour = excluded.segment_hour, byte_offset = excluded.byte_offset",
            params![
                signal.name(),
                position_after.segment_hour.hours_since_epoch(),
                i64::try_from(position_after.byte_offset)?,
            ],
        )?;
        transaction.commit()?;
        Ok(rejected_points)
    }
}

impl CachedRows {
    fn write_records(
        &mut self,
        transaction: &Transaction,
        resource_records: &[ResourceRecords],
    ) -> anyhow::Result<u64> {
        let mut rejected_points = 0;
        let mut catalog_delta = CatalogDelta::default();
        for records in resource_records {
            let days = records.days();
            for &day in &days {
                self.catalog.load_day(transaction, day)?;
            }
            let resource_id = find_or_insert_resource_id(
                transaction,
                &mut self.resource_ids_by_identity_hash,
                &records.resource.service,
                &records.resource.attributes.to_json(),
            )?;
            for &day in &days {
                if self.counted_resource_days.insert((day, resource_id)) {
                    self.catalog.count_attributes(
                        &mut catalog_delta,
                        day,
                        AttributeOwner::Resource,
                        &records.resource.attributes,
                    );
                }
            }
            let mut insert_log = transaction.prepare_cached(
                "INSERT INTO logs (logged_at, resource_id, severity_number, body, trace_id, span_id,
                                   attributes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for log in &records.logs {
                self.catalog.count_attributes(
                    &mut catalog_delta,
                    Day::from_unix_nanos(log.logged_at),
                    AttributeOwner::Log,
                    &log.attributes,
                );
                insert_log.execute(params![
                    log.logged_at,
                    resource_id,
                    log.severity_number.number(),
                    log.body,
                    log.trace_context.trace_id().map(|trace_id| trace_id.0),
                    log.trace_context.span_id().map(|span_id| span_id.0),
                    log.attributes.to_json(),
                ])?;
            }
            let mut insert_span = transaction.prepare_cached(
                "INSERT INTO spans (trace_id, span_id, parent_span_id, resource_id, name, kind,
                                    started_at, duration_ns, status_code, attributes, events)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for span in &records.spans {
                let day = Day::from_unix_nanos(span.started_at);
                self.catalog.count_attributes(
                    &mut catalog_delta,
                    day,
                    AttributeOwner::Span,
                    &span.attributes,
                );
                self.catalog
                    .count_span_name(&mut catalog_delta, day, &span.name);
                insert_span.execute(params![
                    span.trace_id.0,
                    span.span_id.0,
                    span.parent_span_id.map(|id| id.0),
                    resource_id,
                    span.name,
                    span.kind.number(),
                    span.started_at,
                    span.duration_ns,
                    span.status_code.number(),
                    span.attributes.to_json(),
                    serde_json::to_string(&span.events)?,
                ])?;
            }
            rejected_points += self.write_points_and_count_rejected(
                transaction,
                &mut catalog_delta,
                resource_id,
                &records.point_rows_by_metric,
            )?;
        }
        self.catalog.write_delta(transaction, catalog_delta)?;
        Ok(rejected_points)
    }

    fn write_points_and_count_rejected(
        &mut self,
        transaction: &Transaction,
        catalog_delta: &mut CatalogDelta,
        resource_id: ResourceId,
        point_rows_by_metric: &[(&Metric, Vec<PointRow>)],
    ) -> anyhow::Result<u64> {
        let mut rejected_points = 0;
        let mut insert_point = transaction.prepare_cached(
            "INSERT INTO metric_points (metric_series_id, recorded_at, value, histogram)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (metric_series_id, recorded_at) DO UPDATE
             SET value = excluded.value, histogram = excluded.histogram",
        )?;
        for (metric, point_rows) in point_rows_by_metric {
            let stored_series = find_or_insert_series_id(
                transaction,
                &mut self.series_cache,
                resource_id,
                &SeriesIdentity {
                    name: &metric.name,
                    kind: metric.points.kind(),
                    unit: &metric.unit,
                    attributes_json: &metric.attributes.to_json(),
                },
            )?;
            let StoredSeries::Stored(series_id) = stored_series else {
                rejected_points += point_rows.len() as u64;
                continue;
            };
            for point_row in point_rows {
                let day = Day::from_unix_nanos(point_row.recorded_at());
                if self.counted_series_days.insert((day, series_id)) {
                    self.catalog.count_attributes(
                        catalog_delta,
                        day,
                        AttributeOwner::MetricSeries,
                        &metric.attributes,
                    );
                }
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
}
