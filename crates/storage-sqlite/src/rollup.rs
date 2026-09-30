use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};
use std::{fs, io};

use anyhow::{Context, ensure};
use otelo_storage::query::Resolution;
use otelo_storage::{
    Change, Increase, Level, MetricKind, SeriesSteps, StepSummary, Temporality, TimeRange,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::day::Day;
use crate::query::{
    BASELINE_LOOKBACK_NS, WhereClause, metric_kind_from_stored_names, read_series_point,
};
use crate::reader::Reader;
use crate::series::{
    ResourceId, SeriesCache, SeriesId, SeriesIdentity, StoredSeries, find_or_insert_resource_id,
    find_or_insert_series_id,
};
use crate::stored_schema::{StoredSchema, read_stored_schema, set_aside_file_of_another_schema};

pub const ROLLUP_FILE_NAME: &str = "metrics-rollup.sqlite";

const ROLLUP_SCHEMA: &str = include_str!("rollup.sql");
const ROLLUP_SCHEMA_VERSION: i32 = 2;

pub const MINUTE_NS: i64 = 60 * 1_000_000_000;
pub const HOUR_NS: i64 = 60 * MINUTE_NS;

// A batch may reach the writer a while after its points were recorded.
const LATE_BATCH_WAIT_NS: i64 = 2 * MINUTE_NS;

const SUMMARY_COLUMNS: &str = "count, min, max, sum, last, increase, seconds, histogram";

pub enum RollupProgress {
    CaughtUp,
    MoreIsDue,
}

#[derive(Clone, Copy)]
pub enum RollupTable {
    Minutes,
    Hours,
}

impl RollupTable {
    pub const fn from_resolution(resolution: Resolution) -> Option<Self> {
        match resolution {
            Resolution::Raw => None,
            Resolution::Minute => Some(Self::Minutes),
            Resolution::Hour => Some(Self::Hours),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Minutes => "minutes",
            Self::Hours => "hours",
        }
    }

    pub const fn step_ns(self) -> i64 {
        match self {
            Self::Minutes => MINUTE_NS,
            Self::Hours => HOUR_NS,
        }
    }
}

// Series from different day files are one series of the rollups when these match.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SeriesKey {
    service: String,
    resource_attributes_json: String,
    name: String,
    kind: MetricKind,
    unit: String,
    labels_json: String,
}

pub struct Rollups {
    telemetry_directory: PathBuf,
    connection: Connection,
    resource_ids_by_hash: HashMap<i64, ResourceId>,
    series_cache: SeriesCache,
}

impl Rollups {
    pub fn open(telemetry_directory: &Path) -> anyhow::Result<Self> {
        let path = telemetry_directory.join(ROLLUP_FILE_NAME);
        let connection =
            Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        if let StoredSchema::Version(version) = read_stored_schema(&connection)? {
            ensure!(
                version == ROLLUP_SCHEMA_VERSION,
                "{} has the schema version {version}, and this otelo reads version \
                 {ROLLUP_SCHEMA_VERSION}",
                path.display()
            );
        }
        connection
            .execute_batch(ROLLUP_SCHEMA)
            .with_context(|| format!("create the schema in {}", path.display()))?;
        connection.pragma_update(None, "user_version", ROLLUP_SCHEMA_VERSION)?;
        Ok(Self {
            telemetry_directory: telemetry_directory.to_owned(),
            connection,
            resource_ids_by_hash: HashMap::new(),
            series_cache: SeriesCache::default(),
        })
    }

    // At most an hour of minutes and one hour at a time, so the writer takes batches in between.
    pub fn roll_up_next_due(
        &mut self,
        now: i64,
        oldest_raw_at: i64,
    ) -> anyhow::Result<RollupProgress> {
        let minutes_due_until = (now - LATE_BATCH_WAIT_NS).div_euclid(MINUTE_NS) * MINUTE_NS;
        let Some(minutes_rolled_until) = self.read_minutes_rolled_until(oldest_raw_at)? else {
            return Ok(RollupProgress::CaughtUp);
        };
        let minutes_rolled_until = if minutes_rolled_until < minutes_due_until {
            let end_of_hour = (minutes_rolled_until.div_euclid(HOUR_NS) + 1) * HOUR_NS;
            let minute_range =
                TimeRange::new(minutes_rolled_until, end_of_hour.min(minutes_due_until))?;
            self.roll_up_minutes(minute_range)?;
            minute_range.end_at()
        } else {
            minutes_rolled_until
        };

        let hours_due_until = minutes_rolled_until.div_euclid(HOUR_NS) * HOUR_NS;
        let hours_rolled_until = match self.read_rolled_until(RollupTable::Hours)? {
            Some(rolled_until) => rolled_until,
            None => self
                .read_start_of_first_minute()?
                .map_or(hours_due_until, |first_minute_start_at| {
                    first_minute_start_at.div_euclid(HOUR_NS) * HOUR_NS
                }),
        };
        let hours_rolled_until = if hours_rolled_until < hours_due_until {
            self.roll_up_hour(hours_rolled_until)?;
            hours_rolled_until + HOUR_NS
        } else {
            hours_rolled_until
        };
        Ok(
            if minutes_rolled_until < minutes_due_until || hours_rolled_until < hours_due_until {
                RollupProgress::MoreIsDue
            } else {
                RollupProgress::CaughtUp
            },
        )
    }

    fn read_minutes_rolled_until(&self, oldest_raw_at: i64) -> anyhow::Result<Option<i64>> {
        let rolled_until = match self.read_rolled_until(RollupTable::Minutes)? {
            Some(rolled_until) => Some(rolled_until),
            None => self.find_start_of_oldest_day_file()?,
        };
        // A daemon that was down for longer than the raw points are kept starts again at the
        // oldest of them.
        Ok(rolled_until.map(|rolled_until| rolled_until.max(oldest_raw_at)))
    }

    fn find_start_of_oldest_day_file(&self) -> anyhow::Result<Option<i64>> {
        let mut oldest_day = None;
        for entry in fs::read_dir(&self.telemetry_directory)? {
            let file_name = entry?.file_name();
            let day = file_name
                .to_str()
                .filter(|name| name.strip_suffix(".sqlite").is_some())
                .and_then(Day::from_file_name);
            oldest_day = oldest_day.into_iter().chain(day).min();
        }
        Ok(oldest_day.map(Day::start_at))
    }

    fn read_rolled_until(&self, table: RollupTable) -> anyhow::Result<Option<i64>> {
        Ok(self
            .connection
            .prepare_cached("SELECT rolled_until FROM cursors WHERE rollup = ?1")?
            .query_row([table.name()], |row| row.get(0))
            .optional()?)
    }

    fn read_start_of_first_minute(&self) -> anyhow::Result<Option<i64>> {
        Ok(self
            .connection
            .query_row("SELECT min(start_at) FROM minutes", [], |row| row.get(0))?)
    }

    fn roll_up_minutes(&mut self, minute_range: TimeRange) -> anyhow::Result<()> {
        let steps_by_series = self.sum_up_raw_points_by_minute(minute_range)?;
        let transaction = self.connection.transaction()?;
        for (series_key, series_steps) in steps_by_series {
            if series_steps.has_no_point_in_range() {
                continue;
            }
            let stored_resource = find_or_insert_resource_id(
                &transaction,
                &mut self.resource_ids_by_hash,
                &series_key.service,
                &series_key.resource_attributes_json,
            )?;
            let stored_series = find_or_insert_series_id(
                &transaction,
                &mut self.series_cache,
                stored_resource.id(),
                &SeriesIdentity {
                    name: &series_key.name,
                    kind: convert_to_rollup_kind(series_key.kind),
                    unit: &series_key.unit,
                    labels_json: &series_key.labels_json,
                },
            )?;
            let (StoredSeries::Found(series_id) | StoredSeries::Inserted(series_id)) =
                stored_series
            else {
                continue;
            };
            for (minute_start_at, summary) in series_steps.into_summaries_by_step() {
                write_summary(
                    &transaction,
                    RollupTable::Minutes,
                    series_id,
                    minute_start_at,
                    &summary,
                )?;
            }
        }
        write_rolled_until(&transaction, RollupTable::Minutes, minute_range.end_at())?;
        transaction.commit()?;
        Ok(())
    }

    fn sum_up_raw_points_by_minute(
        &self,
        minute_range: TimeRange,
    ) -> anyhow::Result<BTreeMap<SeriesKey, SeriesSteps>> {
        let since = minute_range.start_at().saturating_sub(BASELINE_LOOKBACK_NS);
        let until = minute_range.end_at();
        let reader = Reader::open(&self.telemetry_directory, TimeRange::new(since, until)?)?;
        let mut where_clause = WhereClause::new();
        where_clause.push_condition_with_param("point.recorded_at >= :since", ":since", since);
        where_clause.push_condition_with_param("point.recorded_at < :until", ":until", until);
        let mut steps_by_series: BTreeMap<SeriesKey, SeriesSteps> = BTreeMap::new();
        reader.scan_rows(
            ["", " ORDER BY recorded_at"],
            |day_schema| {
                format!(
                    "SELECT resource.service, resource.attributes, series.name, series.kind,
                            series.temporality, series.unit, series.labels, point.recorded_at,
                            point.value, point.histogram
                     FROM {day_schema}.points point
                     JOIN {day_schema}.series ON series.id = point.series_id
                     JOIN {day_schema}.resources resource ON resource.id = series.resource_id
                     WHERE {}",
                    where_clause.sql_for_day(day_schema)
                )
            },
            &where_clause,
            |row| {
                let kind: String = row.get(3)?;
                let temporality: Option<String> = row.get(4)?;
                let kind = metric_kind_from_stored_names(&kind, temporality.as_deref())?;
                let series_key = SeriesKey {
                    service: row.get(0)?,
                    resource_attributes_json: row.get(1)?,
                    name: row.get(2)?,
                    kind,
                    unit: row.get(5)?,
                    labels_json: row.get(6)?,
                };
                let point = read_series_point(row, 7)?;
                let series_steps = steps_by_series
                    .entry(series_key)
                    .or_insert_with(|| SeriesSteps::new(kind));
                let minute_start_at = point.recorded_at().div_euclid(MINUTE_NS) * MINUTE_NS;
                if point.recorded_at() >= minute_range.start_at() {
                    series_steps.add_point(minute_start_at, point);
                } else {
                    series_steps.add_point_before_range(minute_start_at, point);
                }
                Ok(ControlFlow::Continue(()))
            },
        )?;
        Ok(steps_by_series)
    }

    fn roll_up_hour(&mut self, hour_start_at: i64) -> anyhow::Result<()> {
        let transaction = self.connection.transaction()?;
        let mut summaries_by_series: BTreeMap<SeriesId, StepSummary> = BTreeMap::new();
        {
            let mut select_minutes = transaction.prepare_cached(&format!(
                "SELECT series_id, {SUMMARY_COLUMNS}
                 FROM minutes
                 WHERE start_at >= ?1 AND start_at < ?2
                 ORDER BY start_at"
            ))?;
            let mut rows = select_minutes.query([hour_start_at, hour_start_at + HOUR_NS])?;
            while let Some(row) = rows.next()? {
                let minute_summary = read_summary(row, 1)?;
                match summaries_by_series.entry(row.get(0)?) {
                    Entry::Occupied(entry) => entry.into_mut().add_later_summary(minute_summary),
                    Entry::Vacant(entry) => {
                        entry.insert(minute_summary);
                    }
                }
            }
        }
        for (series_id, summary) in summaries_by_series {
            write_summary(
                &transaction,
                RollupTable::Hours,
                series_id,
                hour_start_at,
                &summary,
            )?;
        }
        write_rolled_until(&transaction, RollupTable::Hours, hour_start_at + HOUR_NS)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_summaries_before(
        &mut self,
        oldest_minute_at: i64,
        oldest_hour_at: i64,
    ) -> anyhow::Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM minutes WHERE start_at < ?1",
            [oldest_minute_at],
        )?;
        transaction.execute("DELETE FROM hours WHERE start_at < ?1", [oldest_hour_at])?;
        transaction.execute_batch(
            "DELETE FROM series WHERE id NOT IN
               (SELECT series_id FROM minutes UNION SELECT series_id FROM hours);
             DELETE FROM resources WHERE id NOT IN (SELECT resource_id FROM series);",
        )?;
        transaction.commit()?;
        self.resource_ids_by_hash.clear();
        self.series_cache = SeriesCache::default();
        Ok(())
    }
}

pub fn set_aside_rollup_file_of_another_schema(telemetry_directory: &Path) -> anyhow::Result<()> {
    let path = telemetry_directory.join(ROLLUP_FILE_NAME);
    if path.is_file() {
        set_aside_file_of_another_schema(&path, ROLLUP_SCHEMA_VERSION)
            .with_context(|| format!("set {} aside", path.display()))?;
    }
    Ok(())
}

// A file set aside takes no more rows, so its newest summary is no newer than its last change.
pub fn delete_rollup_files_set_aside_before(
    telemetry_directory: &Path,
    oldest_hour_at: i64,
) -> io::Result<Vec<String>> {
    let oldest_kept_change =
        UNIX_EPOCH + Duration::from_nanos(u64::try_from(oldest_hour_at).unwrap_or(0));
    let mut deleted_file_names = Vec::new();
    for entry in fs::read_dir(telemetry_directory)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if is_rollup_file_set_aside(&name) && entry.metadata()?.modified()? < oldest_kept_change {
            fs::remove_file(entry.path())?;
            deleted_file_names.push(name);
        }
    }
    deleted_file_names.sort();
    Ok(deleted_file_names)
}

fn is_rollup_file_set_aside(name: &str) -> bool {
    name.strip_prefix(ROLLUP_FILE_NAME)
        .and_then(|rest| rest.strip_prefix(".schema-"))
        .is_some_and(|version| version.parse::<i32>().is_ok())
}

// A summary holds what each step added, whatever the points of the series counted.
const fn convert_to_rollup_kind(kind: MetricKind) -> MetricKind {
    match kind {
        MetricKind::Gauge | MetricKind::UpDown => kind,
        MetricKind::Counter(_) => MetricKind::Counter(Temporality::Delta),
        MetricKind::Histogram(_) => MetricKind::Histogram(Temporality::Delta),
    }
}

fn write_rolled_until(
    transaction: &Transaction,
    table: RollupTable,
    rolled_until: i64,
) -> anyhow::Result<()> {
    transaction
        .prepare_cached("INSERT OR REPLACE INTO cursors (rollup, rolled_until) VALUES (?1, ?2)")?
        .execute(params![table.name(), rolled_until])?;
    Ok(())
}

// Rolling a step up again replaces its row, so it gives the same row.
fn write_summary(
    transaction: &Transaction,
    table: RollupTable,
    series_id: SeriesId,
    start_at: i64,
    summary: &StepSummary,
) -> anyhow::Result<()> {
    let (increase_amount, elapsed_seconds, histogram_json) = match &summary.change {
        Change::Nothing => (None, None, None),
        Change::Increase(increase) => (Some(increase.amount), Some(increase.elapsed_seconds), None),
        Change::Distribution(histogram) => (None, None, Some(serde_json::to_string(histogram)?)),
    };
    transaction
        .prepare_cached(&format!(
            "INSERT OR REPLACE INTO {} (series_id, start_at, {SUMMARY_COLUMNS})
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            table.name()
        ))?
        .execute(params![
            series_id,
            start_at,
            summary.level.count.cast_signed(),
            summary.level.min,
            summary.level.max,
            summary.level.sum,
            summary.level.last,
            increase_amount,
            elapsed_seconds,
            histogram_json,
        ])?;
    Ok(())
}

pub fn read_summary(row: &Row, first_summary_column: usize) -> anyhow::Result<StepSummary> {
    let count: i64 = row.get(first_summary_column)?;
    let increase_amount: Option<f64> = row.get(first_summary_column + 5)?;
    let elapsed_seconds: Option<f64> = row.get(first_summary_column + 6)?;
    let histogram_json: Option<String> = row.get(first_summary_column + 7)?;
    let change = match (increase_amount, elapsed_seconds, histogram_json) {
        (Some(amount), Some(elapsed_seconds), _) => Change::Increase(Increase {
            amount,
            elapsed_seconds,
        }),
        (_, _, Some(json)) => Change::Distribution(Box::new(serde_json::from_str(&json)?)),
        _ => Change::Nothing,
    };
    Ok(StepSummary {
        level: Level {
            count: count.cast_unsigned(),
            min: row.get(first_summary_column + 1)?,
            max: row.get(first_summary_column + 2)?,
            sum: row.get(first_summary_column + 3)?,
            last: row.get(first_summary_column + 4)?,
        },
        change,
    })
}

pub fn summary_columns_of(table_alias: &str) -> String {
    SUMMARY_COLUMNS
        .split(", ")
        .map(|column| format!("{table_alias}.{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}
