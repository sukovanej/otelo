mod calls;
mod catalog;
mod compile;
mod logs;
mod metrics;
mod services;
mod traces;

use std::fmt;
use std::ops::ControlFlow;

use anyhow::bail;
use jiff::Timestamp;
use otelo_indexed_storage::query::{
    AttributeKeys, CallDetail, Calls, LogGroups, Logs, MetricFilter, MetricList, MetricSeries,
    OperationDetail, Resolution, Service, Services, Spans, TargetKey, Trace, Traces,
};
use otelo_indexed_storage::{Error, RangeQueries, Result, SpanId, SpanKind, TraceId};
use otelo_query::{Query, Signal};
use rusqlite::types::Value;
use rusqlite::{Row, ToSql};

pub use compile::InvalidQuery;
pub use metrics::{BASELINE_LOOKBACK_NS, metric_kind_from_stored_names, read_series_point};

use crate::reader::timed_out;
use crate::{Day, Reader};

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

pub const ROLLUP_SCHEMA_NAME: &str = "rollup";

#[derive(Clone, Copy)]
pub struct DaySchema {
    pub day: Day,
}

impl fmt::Display for DaySchema {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "\"{}\"", self.day)
    }
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

    pub fn sql_for_day(&self, day_schema: DaySchema) -> String {
        self.sql_for_schema(&day_schema.to_string())
    }

    pub fn sql_for_rollups(&self) -> String {
        self.sql_for_schema(ROLLUP_SCHEMA_NAME)
    }

    fn sql_for_schema(&self, schema: &str) -> String {
        if self.conditions.is_empty() {
            return "TRUE".into();
        }
        self.conditions.join(" AND ").replace("$day", schema)
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
        [head, tail]: [&str; 2],
        select_for_day: impl Fn(DaySchema) -> String,
        where_clause: &WhereClause,
        mut on_row: impl FnMut(&Row) -> anyhow::Result<ControlFlow<()>>,
    ) -> anyhow::Result<()> {
        if self.days().is_empty() {
            return Ok(());
        }
        let sql = format!(
            "{head}{}{tail}",
            union_day_selects(self.days(), select_for_day)
        );
        let statement_span = new_statement_span(&sql);
        let _entered = statement_span.enter();
        let mut statement = self.connection().prepare(&sql)?;
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

    // The rollups are one file, so they need no SELECT per day.
    pub(crate) fn scan_rollup_rows(
        &self,
        sql: &str,
        where_clause: &WhereClause,
        mut on_row: impl FnMut(&Row) -> anyhow::Result<ControlFlow<()>>,
    ) -> anyhow::Result<()> {
        if !self.has_rollups() {
            return Ok(());
        }
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
        [head, tail]: [&str; 2],
        select_for_day: impl Fn(DaySchema) -> String,
        where_clause: &WhereClause,
    ) -> anyhow::Result<Vec<String>> {
        if self.days().is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "EXPLAIN QUERY PLAN {head}{}{tail}",
            union_day_selects(self.days(), select_for_day)
        );
        let mut statement = self.connection().prepare(&sql)?;
        let steps = statement.query_map(where_clause.params().as_slice(), |row| {
            row.get::<_, String>(3)
        })?;
        Ok(steps.collect::<Result<_, _>>()?)
    }

    pub(crate) fn collect_rows<T>(
        &self,
        head_and_tail: [&str; 2],
        select_for_day: impl Fn(DaySchema) -> String,
        where_clause: &WhereClause,
        mut map_row: impl FnMut(&Row) -> anyhow::Result<T>,
    ) -> anyhow::Result<Vec<T>> {
        let mut rows = Vec::new();
        self.scan_rows(head_and_tail, select_for_day, where_clause, |row| {
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
        db.query.text = sql,
        db.response.returned_rows = tracing::field::Empty,
    )
}

// FTS5 and the row ids only work within one file, so each day file gets a SELECT of its own.
pub fn union_day_selects(days: &[Day], select_for_day: impl Fn(DaySchema) -> String) -> String {
    days.iter()
        .map(|&day| select_for_day(DaySchema { day }))
        .collect::<Vec<_>>()
        .join(" UNION ALL ")
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

    fn list_spans(&self, query: &Query, limit: usize) -> Result<Spans> {
        traces::read_spans(self, query, limit).map_err(classify_query_error)
    }

    fn list_traces(&self, query: &Query, limit: usize) -> Result<Traces> {
        traces::read_traces(self, query, limit).map_err(classify_query_error)
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

    fn get_service(&self, service: &str, step_ns: i64, limit: usize) -> Result<Service> {
        services::summarize_service(self, service, step_ns, limit).map_err(classify_query_error)
    }

    fn get_operation(
        &self,
        service: &str,
        name: &str,
        kind: SpanKind,
        step_ns: i64,
    ) -> Result<OperationDetail> {
        services::summarize_operation(self, service, name, kind, step_ns)
            .map_err(classify_query_error)
    }

    fn list_calls(&self, service: &str, step_ns: i64, limit: usize) -> Result<Calls> {
        calls::summarize_calls(self, service, step_ns, limit).map_err(classify_query_error)
    }

    fn get_call(
        &self,
        service: &str,
        target: &TargetKey,
        summary: &str,
        kind: SpanKind,
        step_ns: i64,
    ) -> Result<CallDetail> {
        calls::summarize_call_operation(self, service, target, summary, kind, step_ns)
            .map_err(classify_query_error)
    }

    fn list_attribute_keys(&self, signal: Signal) -> Result<AttributeKeys> {
        catalog::list_attribute_keys(self, signal).map_err(classify_query_error)
    }

    fn explain_query(&self, query: &Query) -> Result<Vec<String>> {
        explain_query(self, query).map_err(classify_query_error)
    }
}
