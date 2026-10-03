use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::ops::ControlFlow;

use anyhow::{Context, ensure};
use otelo_indexed_storage::query::{
    MAX_BUCKETS_IN_RANGE, MetricFilter, MetricList, MetricSeries, Resolution, SeriesInfo,
};
use otelo_indexed_storage::{
    HistogramPoint, MetricKind, NumberPoint, SeriesPoint, SeriesSteps, StepSummary,
    SummarizedSeries, TimeRange, group_series,
};
use otelo_query::{Query, Signal};

use super::compile::{TableAliases, compile_query};
use super::{WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;
use crate::rollup::{SummaryTable, convert_to_summary_kind, read_summary, summary_columns_of};
use crate::series::MetricSeriesId;

// A counter and a cumulative histogram count from the point before. The apps export every
// minute, so five minutes before the range hold that point.
pub const BASELINE_LOOKBACK_NS: i64 = 5 * 60 * 1_000_000_000;

// A group needs every series of the metric, so the query cannot stop at the limit of the answer.
const MAX_SERIES_IN_A_METRIC_QUERY: usize = 2_000;

const SERIES_TABLE_ALIASES: TableAliases = TableAliases {
    record: "metric_series",
    resource: "resource",
};

struct SeriesDescription {
    service: String,
    kind: MetricKind,
    unit: String,
    attributes_json: String,
    resource_attributes_json: String,
}

impl SeriesDescription {
    // The row has the service, kind, aggregation temporality, unit, attributes, and resource
    // attributes in this order from service_column.
    fn read_from_row(row: &rusqlite::Row, service_column: usize) -> anyhow::Result<Self> {
        let kind: String = row.get(service_column + 1)?;
        let aggregation_temporality: Option<String> = row.get(service_column + 2)?;
        Ok(Self {
            service: row.get(service_column)?,
            kind: metric_kind_from_stored_names(&kind, aggregation_temporality.as_deref())?,
            unit: row.get(service_column + 3)?,
            attributes_json: row.get(service_column + 4)?,
            resource_attributes_json: row.get(service_column + 5)?,
        })
    }

    fn into_summarized_series(
        self,
        summaries_by_step: BTreeMap<i64, StepSummary>,
    ) -> anyhow::Result<SummarizedSeries> {
        Ok(SummarizedSeries {
            service: self.service,
            kind: self.kind,
            unit: self.unit,
            attributes: serde_json::from_str(&self.attributes_json)?,
            resource: serde_json::from_str(&self.resource_attributes_json)?,
            summaries_by_step,
        })
    }
}

fn group_summarized_series(
    range: TimeRange,
    filter: &MetricFilter,
    step_ns: i64,
    resolution: Resolution,
    series: Vec<SummarizedSeries>,
    has_more_series_than_read: bool,
    limit: usize,
) -> MetricSeries {
    let mut groups = group_series(series, &filter.grouping);
    let has_more_groups_than_limit = truncate_to_limit(&mut groups, limit);
    MetricSeries {
        name: filter.name.clone(),
        start_at: timestamp_from_nanos(range.start_at()),
        end_at: timestamp_from_nanos(range.end_at()),
        step_ns,
        resolution,
        groups,
        truncated: has_more_series_than_read || has_more_groups_than_limit,
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
const fn start_of_first_summary(reader: &Reader, table: SummaryTable) -> i64 {
    reader.range().start_at().div_euclid(table.step_ns()) * table.step_ns()
}

const fn round_step_up_to_whole_summaries(step_ns: i64, table: SummaryTable) -> i64 {
    (step_ns + table.step_ns() - 1).div_euclid(table.step_ns()) * table.step_ns()
}

pub(super) fn list_metrics(
    reader: &Reader,
    query: &Query,
    resolution: Resolution,
    limit: usize,
) -> anyhow::Result<MetricList> {
    ensure_metrics_query(query)?;
    let summary_table = SummaryTable::from_resolution(resolution);
    let (points_table, point_alias, instant_column) = summary_table
        .map_or(("metric_points", "metric_point", "recorded_at"), |table| {
            (table.name(), "summary", "start_at")
        });
    let mut where_clause = WhereClause::new();
    where_clause.push_condition(format!(
        "EXISTS (SELECT 1
                 FROM {points_table} {point_alias}
                 WHERE {point_alias}.metric_series_id = metric_series.id
                   AND {point_alias}.{instant_column} >= :since
                   AND {point_alias}.{instant_column} < :until)"
    ));
    where_clause.push_param(
        ":since",
        summary_table.map_or_else(
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
    let sql = format!(
        "SELECT metric_series.name, metric_series.kind, metric_series.aggregation_temporality,
                metric_series.unit, resource.service, metric_series.attributes,
                resource.attributes AS resource_attributes
         FROM metric_series
         JOIN resources resource ON resource.id = metric_series.resource_id
         WHERE {}
         ORDER BY metric_series.name, resource.service, metric_series.attributes
         LIMIT {}",
        where_clause.sql(),
        limit + 1
    );
    let mut series = reader.collect_rows(&sql, &where_clause, |row| {
        let kind: String = row.get(1)?;
        let aggregation_temporality: Option<String> = row.get(2)?;
        let kind = metric_kind_from_stored_names(&kind, aggregation_temporality.as_deref())?;
        let attributes: String = row.get(5)?;
        let resource: String = row.get(6)?;
        Ok(SeriesInfo {
            name: row.get(0)?,
            kind: summary_table.map_or(kind, |_| convert_to_summary_kind(kind)),
            unit: row.get(3)?,
            service: row.get(4)?,
            attributes: serde_json::from_str(&attributes)?,
            resource: serde_json::from_str(&resource)?,
        })
    })?;
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
    SummaryTable::from_resolution(filter.resolution).map_or_else(
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
        "metric_point.recorded_at >= :since",
        ":since",
        range_start_at.saturating_sub(BASELINE_LOOKBACK_NS),
    );
    where_clause.push_condition_with_param(
        "metric_point.recorded_at < :until",
        ":until",
        reader.range().end_at(),
    );
    where_clause.push_condition_with_param(
        "metric_series.name = :name",
        ":name",
        filter.name.clone(),
    );
    compile_query(
        &filter.query,
        SERIES_TABLE_ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_clause,
    )?;
    let sql = format!(
        "SELECT metric_series.id, resource.service, metric_series.kind,
                metric_series.aggregation_temporality, metric_series.unit,
                metric_series.attributes, resource.attributes, metric_point.recorded_at,
                metric_point.value, metric_point.histogram
         FROM metric_series
         JOIN metric_points metric_point ON metric_point.metric_series_id = metric_series.id
         JOIN resources resource ON resource.id = metric_series.resource_id
         WHERE {}
         ORDER BY metric_point.recorded_at",
        where_clause.sql()
    );
    let mut steps_by_series: BTreeMap<MetricSeriesId, (SeriesDescription, SeriesSteps)> =
        BTreeMap::new();
    let mut series_in_range_count = 0;
    let mut has_more_series_than_read = false;
    reader.scan_rows(&sql, &where_clause, |row| {
        let point = read_series_point(row, 7)?;
        let (_, series_steps) = match steps_by_series.entry(row.get(0)?) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let description = SeriesDescription::read_from_row(row, 1)?;
                let series_steps = SeriesSteps::new(description.kind);
                entry.insert((description, series_steps))
            }
        };
        let step_start_at = point.recorded_at().div_euclid(step_ns) * step_ns;
        if point.recorded_at() < range_start_at {
            series_steps.add_point_before_range(step_start_at, point);
            return Ok(ControlFlow::Continue(()));
        }
        // A series with points only before the range is not a series of the range.
        if series_steps.has_no_point_in_range() {
            if series_in_range_count == MAX_SERIES_IN_A_METRIC_QUERY {
                has_more_series_than_read = true;
                return Ok(ControlFlow::Continue(()));
            }
            series_in_range_count += 1;
        }
        series_steps.add_point(step_start_at, point);
        Ok(ControlFlow::Continue(()))
    })?;
    let series = steps_by_series
        .into_values()
        .filter(|(_, series_steps)| !series_steps.has_no_point_in_range())
        .map(|(description, series_steps)| {
            description.into_summarized_series(series_steps.into_summaries_by_step())
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(group_summarized_series(
        reader.range(),
        filter,
        step_ns,
        Resolution::Raw,
        series,
        has_more_series_than_read,
        limit,
    ))
}

fn read_buckets_of_summaries(
    reader: &Reader,
    filter: &MetricFilter,
    table: SummaryTable,
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
    where_clause.push_condition_with_param(
        "metric_series.name = :name",
        ":name",
        filter.name.clone(),
    );
    compile_query(
        &filter.query,
        SERIES_TABLE_ALIASES,
        reader.indexed_attributes(),
        "q",
        &mut where_clause,
    )?;
    let sql = format!(
        "SELECT metric_series.id, resource.service, metric_series.kind,
                metric_series.aggregation_temporality, metric_series.unit,
                metric_series.attributes, resource.attributes, summary.start_at, {}
         FROM metric_series
         JOIN {} summary ON summary.metric_series_id = metric_series.id
         JOIN resources resource ON resource.id = metric_series.resource_id
         WHERE {}
         ORDER BY summary.start_at",
        summary_columns_of("summary"),
        table.name(),
        where_clause.sql()
    );
    let mut summaries_by_series: BTreeMap<
        MetricSeriesId,
        (SeriesDescription, BTreeMap<i64, StepSummary>),
    > = BTreeMap::new();
    let mut has_more_series_than_read = false;
    reader.scan_rows(&sql, &where_clause, |row| {
        let summary_start_at: i64 = row.get(7)?;
        let summary = read_summary(row, 8)?;
        let series_count = summaries_by_series.len();
        let (_, summaries_by_step) = match summaries_by_series.entry(row.get(0)?) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(_) if series_count == MAX_SERIES_IN_A_METRIC_QUERY => {
                has_more_series_than_read = true;
                return Ok(ControlFlow::Continue(()));
            }
            Entry::Vacant(entry) => {
                let mut description = SeriesDescription::read_from_row(row, 1)?;
                description.kind = convert_to_summary_kind(description.kind);
                entry.insert((description, BTreeMap::new()))
            }
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
        .into_values()
        .map(|(description, summaries_by_step)| {
            description.into_summarized_series(summaries_by_step)
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(group_summarized_series(
        reader.range(),
        filter,
        step_ns,
        filter.resolution,
        series,
        has_more_series_than_read,
        limit,
    ))
}
