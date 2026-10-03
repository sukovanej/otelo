mod catalog;
mod compile;
mod logs;
mod metrics;
mod services;
mod span_groups;
mod span_stats;
mod traces;

use std::ops::ControlFlow;

use anyhow::bail;
use jiff::Timestamp;
use otelo_indexed_storage::query::{
    AttributeKeys, LogGroups, Logs, MetricFilter, MetricList, MetricSeries, Resolution, Service,
    Services, SpanGroupingField, SpanGroups, SpanSort, Spans, Trace, Traces,
};
use otelo_indexed_storage::{Error, RangeQueries, Result, SpanId, TraceId};
use otelo_query::{Query, Signal};
use rusqlite::types::Value;
use rusqlite::{Row, ToSql};

pub use compile::InvalidQuery;
pub use metrics::{BASELINE_LOOKBACK_NS, metric_kind_from_stored_names, read_series_point};

use crate::Reader;
use crate::reader::timed_out;

pub fn timestamp_from_nanos(unix_nanos: i64) -> Timestamp {
    Timestamp::from_nanosecond(i128::from(unix_nanos))
        .expect("an i64 of nanoseconds is a valid timestamp")
}

pub fn trace_id_from_blob(blob: Vec<u8>) -> anyhow::Result<TraceId> {
    let bytes = blob
        .try_into()
        .map_err(|blob: Vec<u8>| anyhow::anyhow!("a trace ID of {} bytes", blob.len()))?;
    Ok(TraceId(bytes))
}

pub fn span_id_from_blob(blob: Vec<u8>) -> anyhow::Result<SpanId> {
    let bytes = blob
        .try_into()
        .map_err(|blob: Vec<u8>| anyhow::anyhow!("a span ID of {} bytes", blob.len()))?;
    Ok(SpanId(bytes))
}

pub struct WhereClause {
    conditions: Vec<String>,
    params: Vec<(String, Value)>,
}

impl WhereClause {
    pub const fn new() -> Self {
        Self {
            conditions: Vec::new(),
            params: Vec::new(),
        }
    }

    pub fn within_reader_range(reader: &Reader, instant_column: &str) -> Self {
        let mut where_clause = Self::new();
        where_clause.push_condition_with_param(
            &format!("{instant_column} >= :since"),
            ":since",
            reader.range().start_at(),
        );
        where_clause.push_condition_with_param(
            &format!("{instant_column} < :until"),
            ":until",
            reader.range().end_at(),
        );
        where_clause
    }

    pub fn push_param(&mut self, name: &str, value: impl Into<Value>) {
        self.params.push((name.to_owned(), value.into()));
    }

    pub fn push_condition_with_param(
        &mut self,
        condition: &str,
        name: &str,
        value: impl Into<Value>,
    ) {
        self.conditions.push(condition.to_owned());
        self.params.push((name.to_owned(), value.into()));
    }

    pub fn absorb_params(&mut self, other: Self) {
        for (name, value) in other.params {
            if !self
                .params
                .iter()
                .any(|(existing_name, _)| *existing_name == name)
            {
                self.params.push((name, value));
            }
        }
    }

    pub fn push_condition(&mut self, condition: String) {
        self.conditions.push(condition);
    }

    pub fn sql(&self) -> String {
        if self.conditions.is_empty() {
            return "TRUE".into();
        }
        self.conditions.join(" AND ")
    }

    fn params(&self) -> Vec<(&str, &dyn ToSql)> {
        self.params
            .iter()
            .map(|(name, value)| (name.as_str(), value as &dyn ToSql))
            .collect()
    }
}

impl Reader {
    pub(crate) fn scan_rows(
        &self,
        sql: &str,
        where_clause: &WhereClause,
        mut on_row: impl FnMut(&Row) -> anyhow::Result<ControlFlow<()>>,
    ) -> anyhow::Result<()> {
        let statement_span = new_statement_span(sql);
        let _entered = statement_span.enter();
        let mut statement = self.connection().prepare(sql)?;
        let mut rows = statement.query(where_clause.params().as_slice())?;
        let mut returned_rows = 0_i64;
        while let Some(row) = rows.next()? {
            returned_rows += 1;
            if on_row(row)?.is_break() {
                break;
            }
        }
        statement_span.record("db.response.returned_rows", returned_rows);
        Ok(())
    }

    pub(crate) fn explain_scan(
        &self,
        sql: &str,
        where_clause: &WhereClause,
    ) -> anyhow::Result<Vec<String>> {
        let mut statement = self
            .connection()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let steps = statement.query_map(where_clause.params().as_slice(), |row| {
            row.get::<_, String>(3)
        })?;
        Ok(steps.collect::<Result<_, _>>()?)
    }

    pub(crate) fn collect_rows<T>(
        &self,
        sql: &str,
        where_clause: &WhereClause,
        mut map_row: impl FnMut(&Row) -> anyhow::Result<T>,
    ) -> anyhow::Result<Vec<T>> {
        let mut rows = Vec::new();
        self.scan_rows(sql, where_clause, |row| {
            rows.push(map_row(row)?);
            Ok(ControlFlow::Continue(()))
        })?;
        Ok(rows)
    }
}

// Record db.response.returned_rows as an i64: the OpenTelemetry layer keeps a u64 as text.
pub fn new_statement_span(sql: &str) -> tracing::Span {
    tracing::info_span!(
        "SELECT",
        otel.kind = "client",
        db.system.name = "sqlite",
        db.operation.name = "SELECT",
        db.query.text = sql,
        db.response.returned_rows = tracing::field::Empty,
    )
}

pub fn row_limit_with_one_more(limit: usize) -> anyhow::Result<i64> {
    Ok(i64::try_from(limit)?.saturating_add(1))
}

pub fn truncate_to_limit<T>(rows: &mut Vec<T>, limit: usize) -> bool {
    let truncated = rows.len() > limit;
    rows.truncate(limit);
    truncated
}

fn explain_query(reader: &Reader, query: &Query) -> anyhow::Result<Vec<String>> {
    match query.signal {
        Signal::Logs => logs::explain_logs(reader, query),
        Signal::Spans => traces::explain_spans(reader, query),
        Signal::Metrics => bail!("a plan shows how logs and spans use their indexes"),
    }
}

fn classify_query_error(error: anyhow::Error) -> Error {
    if timed_out(&error) {
        Error::TimedOut
    } else if let Some(invalid) = error.downcast_ref::<InvalidQuery>() {
        Error::InvalidQuery(invalid.0.clone())
    } else {
        Error::Backend(error)
    }
}

impl RangeQueries for Reader {
    fn list_logs(&self, query: &Query, limit: usize) -> Result<Logs> {
        logs::read_logs(self, query, limit).map_err(classify_query_error)
    }

    fn list_log_groups(&self, query: &Query, limit: usize) -> Result<LogGroups> {
        logs::group_logs(self, query, limit).map_err(classify_query_error)
    }

    fn list_spans(&self, query: &Query, sort: SpanSort, limit: usize) -> Result<Spans> {
        traces::read_spans(self, query, sort, limit).map_err(classify_query_error)
    }

    fn list_span_groups(
        &self,
        query: &Query,
        by: &[SpanGroupingField],
        step_ns: i64,
        limit: usize,
    ) -> Result<SpanGroups> {
        span_groups::group_spans(self, query, by, step_ns, limit).map_err(classify_query_error)
    }

    fn list_traces(&self, query: &Query, sort: SpanSort, limit: usize) -> Result<Traces> {
        traces::read_traces(self, query, sort, limit).map_err(classify_query_error)
    }

    fn get_trace(&self, trace_id: TraceId, limit: usize) -> Result<Option<Trace>> {
        traces::read_trace(self, trace_id, limit).map_err(classify_query_error)
    }

    fn list_metrics(
        &self,
        query: &Query,
        resolution: Resolution,
        limit: usize,
    ) -> Result<MetricList> {
        metrics::list_metrics(self, query, resolution, limit).map_err(classify_query_error)
    }

    fn get_metric_series(&self, filter: &MetricFilter, limit: usize) -> Result<MetricSeries> {
        metrics::read_metric_buckets(self, filter, limit).map_err(classify_query_error)
    }

    fn list_services(&self, step_ns: i64, limit: usize) -> Result<Services> {
        services::summarize_services(self, step_ns, limit).map_err(classify_query_error)
    }

    fn get_service(&self, service: &str, step_ns: i64) -> Result<Service> {
        services::summarize_service(self, service, step_ns).map_err(classify_query_error)
    }

    fn list_attribute_keys(&self, signal: Signal) -> Result<AttributeKeys> {
        catalog::list_attribute_keys(self, signal).map_err(classify_query_error)
    }

    fn explain_query(&self, query: &Query) -> Result<Vec<String>> {
        explain_query(self, query).map_err(classify_query_error)
    }
}
