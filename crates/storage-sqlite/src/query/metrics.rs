use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::ops::ControlFlow;

use anyhow::{Context, ensure};
use otelo_query::{Query, Signal};
use otelo_storage::query::{
    Bucket, MAX_BUCKETS_IN_RANGE, MetricFilter, MetricList, MetricSeries, Resolution, Series,
    SeriesInfo,
};
use otelo_storage::{
    HistogramPoint, MetricKind, NumberPoint, SeriesPoint, SeriesSteps, StepSummary,
};

use super::compile::{TableAliases, compile_query};
use super::{ROLLUP_SCHEMA_NAME, WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;
use crate::rollup::{RollupTable, read_summary, summary_columns_of};

// A counter and a cumulative histogram count from the point before. The apps export every
// minute, so five minutes before the range hold that point.
pub const BASELINE_LOOKBACK_NS: i64 = 5 * 60 * 1_000_000_000;

const SERIES_TABLE_ALIASES: TableAliases = TableAliases {
    record: "series",
    resource: "resource",
};

// Series from different day files are one series when these match.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SeriesKey {
    service: String,
    kind: MetricKind,
    unit: String,
    labels_json: String,
    resource_attributes_json: String,
}

impl SeriesKey {
    fn into_series(self, summaries_by_step: BTreeMap<i64, StepSummary>) -> anyhow::Result<Series> {
        Ok(Series {
            service: self.service,
            kind: self.kind,
            unit: self.unit,
            labels: serde_json::from_str(&self.labels_json)?,
            resource: serde_json::from_str(&self.resource_attributes_json)?,
            buckets: summaries_by_step
                .into_iter()
                .map(|(start_at, summary)| {
                    Bucket::from_step_summary(timestamp_from_nanos(start_at), summary)
                })
                .collect(),
        })
    }
}

pub fn metric_kind_from_stored_names(
    kind: &str,
    temporality: Option<&str>,
) -> anyhow::Result<MetricKind> {
    MetricKind::from_name_and_temporality(kind, temporality).with_context(|| {
        format!("a series of the kind {kind:?} with the temporality {temporality:?}")
    })
}

// The row has recorded_at, value, and histogram in this order from recorded_at_column.
pub fn read_series_point(
    row: &rusqlite::Row,
    recorded_at_column: usize,
) -> anyhow::Result<SeriesPoint> {
    let recorded_at: i64 = row.get(recorded_at_column)?;
    let histogram_json: Option<String> = row.get(recorded_at_column + 2)?;
    Ok(match histogram_json {
        Some(json) => SeriesPoint::Histogram(HistogramPoint {
            recorded_at,
            histogram: serde_json::from_str(&json)?,
        }),
        None => SeriesPoint::Number(NumberPoint {
            recorded_at,
            value: row.get(recorded_at_column + 1)?,
        }),
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
    let rollup_table = RollupTable::from_resolution(resolution);
    let (points_table, point_alias, instant_column) = rollup_table
        .map_or(("points", "point", "recorded_at"), |table| {
            (table.name(), "summary", "start_at")
        });
    let mut where_clause = WhereClause::new();
    where_clause.push_condition(format!(
        "EXISTS (SELECT 1
                 FROM $day.{points_table} {point_alias}
                 WHERE {point_alias}.series_id = series.id
                   AND {point_alias}.{instant_column} >= :since
                   AND {point_alias}.{instant_column} < :until)"
    ));
    where_clause.push_param(
        ":since",
        rollup_table.map_or_else(
            || reader.range().start_at(),
            |table| start_of_first_summary(reader, table),
        ),
    );
    where_clause.push_param(":until", reader.range().end_at());
    compile_query(
        query,
        SERIES_TABLE_ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_clause,
    )?;
    let select_series_in_schema = |schema: &dyn fmt::Display, conditions: String| {
        format!(
            "SELECT series.name, series.kind, series.temporality, series.unit, resource.service,
                    series.labels, resource.attributes AS resource_attributes
             FROM {schema}.series
             JOIN {schema}.resources resource ON resource.id = series.resource_id
             WHERE {conditions}"
        )
    };
    let read_series_info = |row: &rusqlite::Row| {
        let kind: String = row.get(1)?;
        let temporality: Option<String> = row.get(2)?;
        let labels: String = row.get(5)?;
        let resource: String = row.get(6)?;
        Ok(SeriesInfo {
            name: row.get(0)?,
            kind: metric_kind_from_stored_names(&kind, temporality.as_deref())?,
            unit: row.get(3)?,
            service: row.get(4)?,
            labels: serde_json::from_str(&labels)?,
            resource: serde_json::from_str(&resource)?,
        })
    };
    let head = "SELECT DISTINCT * FROM (";
    let tail = format!(") ORDER BY name, service, labels LIMIT {}", limit + 1);
    let mut series = if rollup_table.is_some() {
        let sql = format!(
            "{head}{}{tail}",
            select_series_in_schema(&ROLLUP_SCHEMA_NAME, where_clause.sql_for_rollups())
        );
        let mut series = Vec::new();
        reader.scan_rollup_rows(&sql, &where_clause, |row| {
            series.push(read_series_info(row)?);
            Ok(ControlFlow::Continue(()))
        })?;
        series
    } else {
        reader.collect_rows(
            [head, &tail],
            |day_schema| select_series_in_schema(&day_schema, where_clause.sql_for_day(day_schema)),
            &where_clause,
            read_series_info,
        )?
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
        reader.range().length_ns() / filter.step_ns <= MAX_BUCKETS_IN_RANGE,
        "the step makes more than {MAX_BUCKETS_IN_RANGE} buckets in the range; raise the step"
    );
    ensure_metrics_query(&filter.query)?;
    RollupTable::from_resolution(filter.resolution).map_or_else(
        || read_buckets_of_raw_points(reader, filter, limit),
        |table| read_buckets_of_summaries(reader, filter, table, limit),
    )
}

fn read_buckets_of_raw_points(
    reader: &Reader,
    filter: &MetricFilter,
    limit: usize,
) -> anyhow::Result<MetricSeries> {
    let step_ns = filter.step_ns;
    let range_start_at = reader.range().start_at();
    let mut where_clause = WhereClause::new();
    where_clause.push_condition_with_param(
        "point.recorded_at >= :since",
        ":since",
        range_start_at.saturating_sub(BASELINE_LOOKBACK_NS),
    );
    where_clause.push_condition_with_param(
        "point.recorded_at < :until",
        ":until",
        reader.range().end_at(),
    );
    where_clause.push_condition_with_param("series.name = :name", ":name", filter.name.clone());
    compile_query(
        &filter.query,
        SERIES_TABLE_ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_clause,
    )?;
    let mut steps_by_series: BTreeMap<SeriesKey, SeriesSteps> = BTreeMap::new();
    let mut series_in_range_count = 0;
    let mut truncated = false;
    reader.scan_rows(
        ["", " ORDER BY recorded_at"],
        |day_schema| {
            format!(
                "SELECT resource.service, series.kind, series.temporality, series.unit,
                        series.labels, resource.attributes, point.recorded_at, point.value,
                        point.histogram
                 FROM {day_schema}.points point
                 JOIN {day_schema}.series ON series.id = point.series_id
                 JOIN {day_schema}.resources resource ON resource.id = series.resource_id
                 WHERE {}",
                where_clause.sql_for_day(day_schema)
            )
        },
        &where_clause,
        |row| {
            let kind: String = row.get(1)?;
            let temporality: Option<String> = row.get(2)?;
            let kind = metric_kind_from_stored_names(&kind, temporality.as_deref())?;
            let series_key = SeriesKey {
                service: row.get(0)?,
                kind,
                unit: row.get(3)?,
                labels_json: row.get(4)?,
                resource_attributes_json: row.get(5)?,
            };
            let point = read_series_point(row, 6)?;
            let series_steps = steps_by_series
                .entry(series_key)
                .or_insert_with(|| SeriesSteps::new(kind));
            let step_start_at = point.recorded_at().div_euclid(step_ns) * step_ns;
            if point.recorded_at() < range_start_at {
                series_steps.add_point_before_range(step_start_at, point);
                return Ok(ControlFlow::Continue(()));
            }
            // A series with points only before the range is not a series of the range.
            if series_steps.has_no_point_in_range() {
                if series_in_range_count == limit {
                    truncated = true;
                    return Ok(ControlFlow::Continue(()));
                }
                series_in_range_count += 1;
            }
            series_steps.add_point(step_start_at, point);
            Ok(ControlFlow::Continue(()))
        },
    )?;
    let series = steps_by_series
        .into_iter()
        .filter(|(_, series_steps)| !series_steps.has_no_point_in_range())
        .map(|(series_key, series_steps)| {
            series_key.into_series(series_steps.into_summaries_by_step())
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(MetricSeries {
        name: filter.name.clone(),
        step_ns,
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
    let step_ns = round_step_up_to_whole_summaries(filter.step_ns, table);
    let mut where_clause = WhereClause::new();
    where_clause.push_condition_with_param(
        "summary.start_at >= :since",
        ":since",
        start_of_first_summary(reader, table),
    );
    where_clause.push_condition_with_param(
        "summary.start_at < :until",
        ":until",
        reader.range().end_at(),
    );
    where_clause.push_condition_with_param("series.name = :name", ":name", filter.name.clone());
    compile_query(
        &filter.query,
        SERIES_TABLE_ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_clause,
    )?;
    let sql = format!(
        "SELECT resource.service, series.kind, series.temporality, series.unit, series.labels,
                resource.attributes, summary.start_at, {}
         FROM {ROLLUP_SCHEMA_NAME}.{} summary
         JOIN {ROLLUP_SCHEMA_NAME}.series ON series.id = summary.series_id
         JOIN {ROLLUP_SCHEMA_NAME}.resources resource ON resource.id = series.resource_id
         WHERE {}
         ORDER BY summary.start_at",
        summary_columns_of("summary"),
        table.name(),
        where_clause.sql_for_rollups()
    );
    let mut summaries_by_series: BTreeMap<SeriesKey, BTreeMap<i64, StepSummary>> = BTreeMap::new();
    let mut truncated = false;
    reader.scan_rollup_rows(&sql, &where_clause, |row| {
        let kind: String = row.get(1)?;
        let temporality: Option<String> = row.get(2)?;
        let series_key = SeriesKey {
            service: row.get(0)?,
            kind: metric_kind_from_stored_names(&kind, temporality.as_deref())?,
            unit: row.get(3)?,
            labels_json: row.get(4)?,
            resource_attributes_json: row.get(5)?,
        };
        let summary_start_at: i64 = row.get(6)?;
        let summary = read_summary(row, 7)?;
        let series_count = summaries_by_series.len();
        let summaries_by_step = match summaries_by_series.entry(series_key) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(_) if series_count == limit => {
                truncated = true;
                return Ok(ControlFlow::Continue(()));
            }
            Entry::Vacant(entry) => entry.insert(BTreeMap::new()),
        };
        match summaries_by_step.entry(summary_start_at.div_euclid(step_ns) * step_ns) {
            Entry::Occupied(entry) => entry.into_mut().add_later_summary(summary),
            Entry::Vacant(entry) => {
                entry.insert(summary);
            }
        }
        Ok(ControlFlow::Continue(()))
    })?;
    let series = summaries_by_series
        .into_iter()
        .map(|(series_key, summaries_by_step)| series_key.into_series(summaries_by_step))
        .collect::<anyhow::Result<_>>()?;
    Ok(MetricSeries {
        name: filter.name.clone(),
        step_ns,
        resolution: filter.resolution,
        series,
        truncated,
    })
}
