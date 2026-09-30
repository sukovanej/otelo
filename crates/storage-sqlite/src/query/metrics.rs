use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use anyhow::{Context, ensure};
use otelo_query::{Query, Signal};
use otelo_storage::query::{
    Bucket, MAX_BUCKETS, MetricFilter, MetricList, MetricSeries, Resolution, Series, SeriesInfo,
};
use otelo_storage::{Histogram, MetricKind, NumberPoint, SeriesSteps, StepSummary};

use super::compile::{TableAliases, compile_query};
use super::{WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;
use crate::rollup::{RollupTable, read_summary, summary_columns_of};

// A counter and a cumulative histogram count from the point before. The apps export every
// minute, so five minutes before the range hold that point.
pub const BASELINE_LOOKBACK_NS: i64 = 5 * 60 * 1_000_000_000;

const ROLLUP_SCHEMA_NAME: &str = "rollup";

const ALIASES: TableAliases = TableAliases {
    record: "s",
    resource: "r",
};

// Series from different day files are one series when these match.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SeriesKey {
    service: String,
    kind: MetricKind,
    unit: String,
    labels: String,
    resource: String,
}

impl SeriesKey {
    fn into_series(self, steps: BTreeMap<i64, StepSummary>) -> anyhow::Result<Series> {
        Ok(Series {
            service: self.service,
            kind: self.kind,
            unit: self.unit,
            labels: serde_json::from_str(&self.labels)?,
            resource: serde_json::from_str(&self.resource)?,
            buckets: steps
                .into_iter()
                .map(|(start_at, summary)| Bucket::of_step(timestamp_from_nanos(start_at), summary))
                .collect(),
        })
    }
}

pub fn kind_of_series(name: &str, temporality: Option<&str>) -> anyhow::Result<MetricKind> {
    MetricKind::from_name_and_temporality(name, temporality).with_context(|| {
        format!("a series of the kind {name:?} with the temporality {temporality:?}")
    })
}

fn ensure_metrics_query(query: &Query) -> anyhow::Result<()> {
    ensure!(
        query.signal == Signal::Metrics,
        "the query is over {}, not metrics",
        query.signal
    );
    Ok(())
}

// A summary covers a whole minute or hour, so the one the range starts in is part of it.
const fn start_of_first_summary(reader: &Reader, table: RollupTable) -> i64 {
    reader.range().start_at().div_euclid(table.step_ns()) * table.step_ns()
}

const fn round_step_up_to_whole_summaries(step_ns: i64, table: RollupTable) -> i64 {
    (step_ns + table.step_ns() - 1).div_euclid(table.step_ns()) * table.step_ns()
}

pub(super) fn list_metrics(
    reader: &Reader,
    query: &Query,
    resolution: Resolution,
    limit: usize,
) -> anyhow::Result<MetricList> {
    ensure_metrics_query(query)?;
    let table = RollupTable::of_resolution(resolution);
    let (points, time) = table.map_or(("points", "ts"), |table| (table.name(), "start"));
    let mut where_ = WhereClause::new();
    where_.push_clause(format!(
        "EXISTS (SELECT 1 FROM $day.{points} p
                 WHERE p.series_id = s.id AND p.{time} >= :since AND p.{time} < :until)"
    ));
    where_.push_param(
        ":since",
        table.map_or_else(
            || reader.range().start_at(),
            |table| start_of_first_summary(reader, table),
        ),
    );
    where_.push_param(":until", reader.range().end_at());
    compile_query(
        query,
        ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_,
    )?;
    let select_for_schema = |schema: &str| {
        format!(
            "SELECT s.name, s.kind, s.temporality, s.unit, r.service, s.labels,
                    r.attributes AS resource
             FROM {schema}.series s JOIN {schema}.resources r ON r.id = s.resource_id
             WHERE {}",
            where_.sql_for_day(schema)
        )
    };
    let map_row = |row: &rusqlite::Row| {
        let kind: String = row.get(1)?;
        let temporality: Option<String> = row.get(2)?;
        let labels: String = row.get(5)?;
        let resource: String = row.get(6)?;
        Ok(SeriesInfo {
            name: row.get(0)?,
            kind: kind_of_series(&kind, temporality.as_deref())?,
            unit: row.get(3)?,
            service: row.get(4)?,
            labels: serde_json::from_str(&labels)?,
            resource: serde_json::from_str(&resource)?,
        })
    };
    let head = "SELECT DISTINCT * FROM (";
    let tail = format!(") ORDER BY name, service, labels LIMIT {}", limit + 1);
    let mut series = if table.is_some() {
        let sql = format!("{head}{}{tail}", select_for_schema(ROLLUP_SCHEMA_NAME));
        let mut series = Vec::new();
        reader.scan_rollup_rows(&sql, &where_, |row| {
            series.push(map_row(row)?);
            Ok(true)
        })?;
        series
    } else {
        reader.collect_rows([head, &tail], select_for_schema, &where_, map_row)?
    };
    let truncated = truncate_to_limit(&mut series, limit);
    Ok(MetricList { series, truncated })
}

pub(super) fn read_metric_buckets(
    reader: &Reader,
    filter: &MetricFilter,
    limit: usize,
) -> anyhow::Result<MetricSeries> {
    ensure!(filter.step_ns > 0, "the step has to be longer than zero");
    ensure!(
        reader.range().length() / filter.step_ns <= MAX_BUCKETS,
        "the step makes more than {MAX_BUCKETS} buckets in the range; raise the step"
    );
    ensure_metrics_query(&filter.query)?;
    RollupTable::of_resolution(filter.resolution).map_or_else(
        || read_buckets_of_raw_points(reader, filter, limit),
        |table| read_buckets_of_summaries(reader, filter, table, limit),
    )
}

fn read_buckets_of_raw_points(
    reader: &Reader,
    filter: &MetricFilter,
    limit: usize,
) -> anyhow::Result<MetricSeries> {
    let step = filter.step_ns;
    let range_start_at = reader.range().start_at();
    let mut where_ = WhereClause::new();
    where_.push_clause_with_param(
        "p.ts >= :since",
        ":since",
        range_start_at.saturating_sub(BASELINE_LOOKBACK_NS),
    );
    where_.push_clause_with_param("p.ts < :until", ":until", reader.range().end_at());
    where_.push_clause_with_param("s.name = :name", ":name", filter.name.clone());
    compile_query(
        &filter.query,
        ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_,
    )?;
    let mut series: BTreeMap<SeriesKey, SeriesSteps> = BTreeMap::new();
    let mut series_in_range = 0;
    let mut truncated = false;
    reader.scan_rows(
        ["", " ORDER BY ts"],
        |day| {
            format!(
                "SELECT r.service, s.kind, s.temporality, s.unit, s.labels, r.attributes, p.ts,
                        p.value, p.histogram
                 FROM {day}.points p
                 JOIN {day}.series s ON s.id = p.series_id
                 JOIN {day}.resources r ON r.id = s.resource_id
                 WHERE {}",
                where_.sql_for_day(day)
            )
        },
        &where_,
        |row| {
            let kind: String = row.get(1)?;
            let temporality: Option<String> = row.get(2)?;
            let kind = kind_of_series(&kind, temporality.as_deref())?;
            let key = SeriesKey {
                service: row.get(0)?,
                kind,
                unit: row.get(3)?,
                labels: row.get(4)?,
                resource: row.get(5)?,
            };
            let point = NumberPoint {
                recorded_at: row.get(6)?,
                value: row.get(7)?,
            };
            let histogram: Option<String> = row.get(8)?;
            let histogram = histogram
                .map(|json| serde_json::from_str::<Histogram>(&json))
                .transpose()?;
            let steps = series
                .entry(key)
                .or_insert_with(|| SeriesSteps::of_kind(kind));
            let step = point.recorded_at.div_euclid(step) * step;
            if point.recorded_at < range_start_at {
                steps.add_point_before_range(step, point, histogram);
                return Ok(true);
            }
            // A series with points only before the range is not a series of the range.
            if steps.has_no_point_in_range() {
                if series_in_range == limit {
                    truncated = true;
                    return Ok(true);
                }
                series_in_range += 1;
            }
            steps.add_point(step, point, histogram);
            Ok(true)
        },
    )?;
    let series = series
        .into_iter()
        .filter(|(_, steps)| !steps.has_no_point_in_range())
        .map(|(key, steps)| key.into_series(steps.into_summaries_by_step()))
        .collect::<anyhow::Result<_>>()?;
    Ok(MetricSeries {
        name: filter.name.clone(),
        step_ns: step,
        resolution: Resolution::Raw,
        series,
        truncated,
    })
}

fn read_buckets_of_summaries(
    reader: &Reader,
    filter: &MetricFilter,
    table: RollupTable,
    limit: usize,
) -> anyhow::Result<MetricSeries> {
    let step = round_step_up_to_whole_summaries(filter.step_ns, table);
    let mut where_ = WhereClause::new();
    where_.push_clause_with_param(
        "m.start >= :since",
        ":since",
        start_of_first_summary(reader, table),
    );
    where_.push_clause_with_param("m.start < :until", ":until", reader.range().end_at());
    where_.push_clause_with_param("s.name = :name", ":name", filter.name.clone());
    compile_query(
        &filter.query,
        ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_,
    )?;
    let sql = format!(
        "SELECT r.service, s.kind, s.temporality, s.unit, s.labels, r.attributes, m.start, {}
         FROM {ROLLUP_SCHEMA_NAME}.{} m
         JOIN {ROLLUP_SCHEMA_NAME}.series s ON s.id = m.series_id
         JOIN {ROLLUP_SCHEMA_NAME}.resources r ON r.id = s.resource_id
         WHERE {} ORDER BY m.start",
        summary_columns_of("m"),
        table.name(),
        where_.sql_for_day(ROLLUP_SCHEMA_NAME)
    );
    let mut series: BTreeMap<SeriesKey, BTreeMap<i64, StepSummary>> = BTreeMap::new();
    let mut truncated = false;
    reader.scan_rollup_rows(&sql, &where_, |row| {
        let kind: String = row.get(1)?;
        let temporality: Option<String> = row.get(2)?;
        let key = SeriesKey {
            service: row.get(0)?,
            kind: kind_of_series(&kind, temporality.as_deref())?,
            unit: row.get(3)?,
            labels: row.get(4)?,
            resource: row.get(5)?,
        };
        let start: i64 = row.get(6)?;
        let summary = read_summary(row, 7)?;
        let len = series.len();
        let steps = match series.entry(key) {
            Entry::Occupied(steps) => steps.into_mut(),
            Entry::Vacant(_) if len == limit => {
                truncated = true;
                return Ok(true);
            }
            Entry::Vacant(steps) => steps.insert(BTreeMap::new()),
        };
        match steps.entry(start.div_euclid(step) * step) {
            Entry::Occupied(of_step) => of_step.into_mut().add_later_summary(summary),
            Entry::Vacant(of_step) => {
                of_step.insert(summary);
            }
        }
        Ok(true)
    })?;
    let series = series
        .into_iter()
        .map(|(key, steps)| key.into_series(steps))
        .collect::<anyhow::Result<_>>()?;
    Ok(MetricSeries {
        name: filter.name.clone(),
        step_ns: step,
        resolution: filter.resolution,
        series,
        truncated,
    })
}
