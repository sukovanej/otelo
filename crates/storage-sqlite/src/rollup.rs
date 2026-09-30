use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};
use otelo_storage::query::Resolution;
use otelo_storage::{
    Change, Histogram, Increase, Level, MetricKind, NumberPoint, SeriesSteps, StepSummary,
    Temporality, TimeRange,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::day::Day;
use crate::query::{BASELINE_LOOKBACK_NS, WhereClause, kind_of_series};
use crate::reader::Reader;
use crate::series::{
    SeriesCache, SeriesIdentity, StoredSeries, find_or_insert_resource_id, find_or_insert_series_id,
};
use crate::stored_schema::{StoredSchema, read_stored_schema};

pub const ROLLUP_FILE_NAME: &str = "metrics-rollup.sqlite";

const SCHEMA: &str = include_str!("rollup.sql");
const SCHEMA_VERSION: i32 = 1;

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
    pub const fn of_resolution(resolution: Resolution) -> Option<Self> {
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
    resource: String,
    name: String,
    kind: MetricKind,
    unit: String,
    labels: String,
}

pub struct Rollups {
    telemetry_dir: PathBuf,
    conn: Connection,
    resource_ids_by_hash: HashMap<i64, i64>,
    series: SeriesCache,
}

impl Rollups {
    pub fn open(telemetry_dir: &Path) -> anyhow::Result<Self> {
        let path = telemetry_dir.join(ROLLUP_FILE_NAME);
        let conn = Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        if let StoredSchema::Version(version) = read_stored_schema(&conn)? {
            ensure!(
                version == SCHEMA_VERSION,
                "{} has the schema version {version}, and this otelo reads version \
                 {SCHEMA_VERSION}",
                path.display()
            );
        }
        conn.execute_batch(SCHEMA)
            .with_context(|| format!("create the schema in {}", path.display()))?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self {
            telemetry_dir: telemetry_dir.to_owned(),
            conn,
            resource_ids_by_hash: HashMap::new(),
            series: SeriesCache::default(),
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
            let minutes = TimeRange::new(minutes_rolled_until, end_of_hour.min(minutes_due_until))?;
            self.roll_up_minutes(minutes)?;
            minutes.end_at()
        } else {
            minutes_rolled_until
        };

        let hours_due_until = minutes_rolled_until.div_euclid(HOUR_NS) * HOUR_NS;
        let hours_rolled_until = match self.read_rolled_until(RollupTable::Hours)? {
            Some(rolled_until) => rolled_until,
            None => self
                .read_start_of_first_minute()?
                .map_or(hours_due_until, |start| start.div_euclid(HOUR_NS) * HOUR_NS),
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
        let mut oldest = None;
        for entry in std::fs::read_dir(&self.telemetry_dir)? {
            let name = entry?.file_name();
            let day = name
                .to_str()
                .filter(|name| name.strip_suffix(".sqlite").is_some())
                .and_then(Day::from_file_name);
            oldest = oldest.into_iter().chain(day).min();
        }
        Ok(oldest.map(Day::start))
    }

    fn read_rolled_until(&self, table: RollupTable) -> anyhow::Result<Option<i64>> {
        Ok(self
            .conn
            .prepare_cached("SELECT rolled_until FROM cursors WHERE rollup = ?1")?
            .query_row([table.name()], |row| row.get(0))
            .optional()?)
    }

    fn read_start_of_first_minute(&self) -> anyhow::Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT min(start) FROM minutes", [], |row| row.get(0))?)
    }

    fn roll_up_minutes(&mut self, minutes: TimeRange) -> anyhow::Result<()> {
        let steps_by_series = self.sum_up_raw_points_by_minute(minutes)?;
        let tx = self.conn.transaction()?;
        for (key, steps) in steps_by_series {
            if steps.has_no_point_in_range() {
                continue;
            }
            let resource = find_or_insert_resource_id(
                &tx,
                &mut self.resource_ids_by_hash,
                &key.service,
                &key.resource,
            )?;
            let series = find_or_insert_series_id(
                &tx,
                &mut self.series,
                resource.id(),
                &SeriesIdentity {
                    name: &key.name,
                    kind: kind_of_rollup(key.kind),
                    unit: &key.unit,
                    labels_json: &key.labels,
                },
            )?;
            let (StoredSeries::Found(series_id) | StoredSeries::Inserted(series_id)) = series
            else {
                continue;
            };
            for (start, summary) in steps.into_summaries_by_step() {
                write_summary(&tx, RollupTable::Minutes, series_id, start, &summary)?;
            }
        }
        write_rolled_until(&tx, RollupTable::Minutes, minutes.end_at())?;
        tx.commit()?;
        Ok(())
    }

    fn sum_up_raw_points_by_minute(
        &self,
        minutes: TimeRange,
    ) -> anyhow::Result<BTreeMap<SeriesKey, SeriesSteps>> {
        let since = minutes.start_at().saturating_sub(BASELINE_LOOKBACK_NS);
        let until = minutes.end_at();
        let reader = Reader::open(&self.telemetry_dir, TimeRange::new(since, until)?)?;
        let mut range = WhereClause::new();
        range.push_clause_with_param("p.ts >= :since", ":since", since);
        range.push_clause_with_param("p.ts < :until", ":until", until);
        let mut steps_by_series: BTreeMap<SeriesKey, SeriesSteps> = BTreeMap::new();
        reader.scan_rows(
            ["", " ORDER BY ts"],
            |day| {
                format!(
                    "SELECT r.service, r.attributes, s.name, s.kind, s.temporality, s.unit,
                            s.labels, p.ts, p.value, p.histogram
                     FROM {day}.points p
                     JOIN {day}.series s ON s.id = p.series_id
                     JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    range.sql_for_day(day)
                )
            },
            &range,
            |row| {
                let kind: String = row.get(3)?;
                let temporality: Option<String> = row.get(4)?;
                let kind = kind_of_series(&kind, temporality.as_deref())?;
                let key = SeriesKey {
                    service: row.get(0)?,
                    resource: row.get(1)?,
                    name: row.get(2)?,
                    kind,
                    unit: row.get(5)?,
                    labels: row.get(6)?,
                };
                let point = NumberPoint {
                    recorded_at: row.get(7)?,
                    value: row.get(8)?,
                };
                let histogram: Option<String> = row.get(9)?;
                let histogram = histogram
                    .map(|json| serde_json::from_str::<Histogram>(&json))
                    .transpose()?;
                let steps = steps_by_series
                    .entry(key)
                    .or_insert_with(|| SeriesSteps::of_kind(kind));
                let minute = point.recorded_at.div_euclid(MINUTE_NS) * MINUTE_NS;
                if point.recorded_at >= minutes.start_at() {
                    steps.add_point(minute, point, histogram);
                } else {
                    steps.add_point_before_range(minute, point, histogram);
                }
                Ok(true)
            },
        )?;
        Ok(steps_by_series)
    }

    fn roll_up_hour(&mut self, hour_start_at: i64) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        let mut summaries: BTreeMap<i64, StepSummary> = BTreeMap::new();
        {
            let mut minutes = tx.prepare_cached(&format!(
                "SELECT series_id, {SUMMARY_COLUMNS} FROM minutes
                 WHERE start >= ?1 AND start < ?2 ORDER BY start"
            ))?;
            let mut rows = minutes.query([hour_start_at, hour_start_at + HOUR_NS])?;
            while let Some(row) = rows.next()? {
                let summary = read_summary(row, 1)?;
                match summaries.entry(row.get(0)?) {
                    Entry::Occupied(of_hour) => of_hour.into_mut().add_later_summary(summary),
                    Entry::Vacant(of_hour) => {
                        of_hour.insert(summary);
                    }
                }
            }
        }
        for (series_id, summary) in summaries {
            write_summary(&tx, RollupTable::Hours, series_id, hour_start_at, &summary)?;
        }
        write_rolled_until(&tx, RollupTable::Hours, hour_start_at + HOUR_NS)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_summaries_before(
        &mut self,
        oldest_minute_at: i64,
        oldest_hour_at: i64,
    ) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM minutes WHERE start < ?1", [oldest_minute_at])?;
        tx.execute("DELETE FROM hours WHERE start < ?1", [oldest_hour_at])?;
        tx.execute_batch(
            "DELETE FROM series WHERE id NOT IN
               (SELECT series_id FROM minutes UNION SELECT series_id FROM hours);
             DELETE FROM resources WHERE id NOT IN (SELECT resource_id FROM series);",
        )?;
        tx.commit()?;
        self.resource_ids_by_hash.clear();
        self.series = SeriesCache::default();
        Ok(())
    }
}

// A summary holds what each step added, whatever the points of the series counted.
const fn kind_of_rollup(kind: MetricKind) -> MetricKind {
    match kind {
        MetricKind::Gauge | MetricKind::UpDown => kind,
        MetricKind::Counter(_) => MetricKind::Counter(Temporality::Delta),
        MetricKind::Histogram(_) => MetricKind::Histogram(Temporality::Delta),
    }
}

fn write_rolled_until(
    tx: &Transaction,
    table: RollupTable,
    rolled_until: i64,
) -> anyhow::Result<()> {
    tx.prepare_cached("INSERT OR REPLACE INTO cursors (rollup, rolled_until) VALUES (?1, ?2)")?
        .execute(params![table.name(), rolled_until])?;
    Ok(())
}

// Rolling a step up again replaces its row, so it gives the same row.
fn write_summary(
    tx: &Transaction,
    table: RollupTable,
    series_id: i64,
    start: i64,
    summary: &StepSummary,
) -> anyhow::Result<()> {
    let (increase, seconds, histogram) = match &summary.change {
        Change::Nothing => (None, None, None),
        Change::Increase(increase) => (Some(increase.amount), Some(increase.seconds), None),
        Change::Distribution(histogram) => (None, None, Some(serde_json::to_string(histogram)?)),
    };
    tx.prepare_cached(&format!(
        "INSERT OR REPLACE INTO {} (series_id, start, {SUMMARY_COLUMNS})
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        table.name()
    ))?
    .execute(params![
        series_id,
        start,
        summary.level.count.cast_signed(),
        summary.level.min,
        summary.level.max,
        summary.level.sum,
        summary.level.last,
        increase,
        seconds,
        histogram,
    ])?;
    Ok(())
}

pub fn read_summary(row: &Row, first_summary_column: usize) -> anyhow::Result<StepSummary> {
    let first = first_summary_column;
    let count: i64 = row.get(first)?;
    let increase: Option<f64> = row.get(first + 5)?;
    let seconds: Option<f64> = row.get(first + 6)?;
    let histogram: Option<String> = row.get(first + 7)?;
    let change = match (increase, seconds, histogram) {
        (Some(amount), Some(seconds), _) => Change::Increase(Increase { amount, seconds }),
        (_, _, Some(json)) => Change::Distribution(Box::new(serde_json::from_str(&json)?)),
        _ => Change::Nothing,
    };
    Ok(StepSummary {
        level: Level {
            count: count.cast_unsigned(),
            min: row.get(first + 1)?,
            max: row.get(first + 2)?,
            sum: row.get(first + 3)?,
            last: row.get(first + 4)?,
        },
        change,
    })
}

pub fn summary_columns_of(alias: &str) -> String {
    SUMMARY_COLUMNS
        .split(", ")
        .map(|column| format!("{alias}.{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}
