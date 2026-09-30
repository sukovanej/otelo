use std::collections::HashMap;

use anyhow::ensure;
use otelo_query::{BuiltinField, Expression, Field, Operator, Query, Signal};
use otelo_storage::query::{Spans, Trace, TraceSpan, TraceSummary, Traces};
use otelo_storage::{Attributes, SpanKind, SpanStatus, TraceId};
use rusqlite::Row;

use super::compile::{TableAliases, compile_query};
use super::services::SpanLocation;
use super::{
    DaySchema, WhereClause, new_statement_span, span_id_from_blob, timestamp_from_nanos,
    trace_id_from_blob, truncate_to_limit, union_day_selects,
};
use crate::{Day, Reader};

#[derive(Clone, Copy)]
struct TraceStats {
    spans: u64,
    failed: bool,
}

struct RootSpan {
    trace_id: TraceId,
    started_at: i64,
    service: String,
    name: String,
    kind: SpanKind,
    duration_ns: i64,
    attributes: Attributes,
    resource: Attributes,
}

const SPAN_COLUMNS: &str = "span.trace_id, span.span_id, span.parent_span_id, resource.service,
    span.name, span.kind, span.started_at, span.duration_ns, span.status, span.attributes,
    span.events, resource.attributes AS resource_attributes";

fn select_spans(where_clause: &WhereClause) -> impl Fn(DaySchema) -> String {
    move |day_schema| {
        format!(
            "SELECT {SPAN_COLUMNS}
             FROM {day_schema}.spans span
             JOIN {day_schema}.resources resource ON resource.id = span.resource_id
             WHERE {}",
            where_clause.sql_for_day(day_schema)
        )
    }
}

impl Reader {
    pub(super) fn read_spans_at(
        &self,
        locations: &[SpanLocation],
    ) -> anyhow::Result<Vec<TraceSpan>> {
        let mut rowids_by_day: HashMap<Day, Vec<String>> = HashMap::new();
        for location in locations {
            rowids_by_day
                .entry(location.day)
                .or_default()
                .push(location.rowid.to_string());
        }
        let mut spans = Vec::with_capacity(locations.len());
        for (day, rowids) in rowids_by_day {
            let day_schema = DaySchema { day };
            let sql = format!(
                "SELECT {SPAN_COLUMNS}
                 FROM {day_schema}.spans span
                 JOIN {day_schema}.resources resource ON resource.id = span.resource_id
                 WHERE span.rowid IN ({})",
                rowids.join(", ")
            );
            let statement_span = new_statement_span(&sql);
            let _entered = statement_span.enter();
            let mut statement = self.connection().prepare(&sql)?;
            let mut rows = statement.query([])?;
            let mut returned_rows = 0_i64;
            while let Some(row) = rows.next()? {
                returned_rows += 1;
                spans.push(trace_span_from_row(row)?);
            }
            statement_span.record("db.response.returned_rows", returned_rows);
        }
        spans.sort_by_key(|span| std::cmp::Reverse(span.started_at));
        Ok(spans)
    }
}

fn trace_span_from_row(row: &Row) -> anyhow::Result<TraceSpan> {
    let parent_span_id: Option<Vec<u8>> = row.get(2)?;
    let attributes: String = row.get(9)?;
    let events: String = row.get(10)?;
    let resource: String = row.get(11)?;
    Ok(TraceSpan {
        trace_id: trace_id_from_blob(row.get(0)?)?,
        span_id: span_id_from_blob(row.get(1)?)?,
        parent_span_id: parent_span_id.map(span_id_from_blob).transpose()?,
        service: row.get(3)?,
        name: row.get(4)?,
        kind: SpanKind::from_number(row.get(5)?),
        started_at: timestamp_from_nanos(row.get(6)?),
        duration_ns: row.get(7)?,
        status: SpanStatus::from_number(row.get(8)?),
        attributes: serde_json::from_str(&attributes)?,
        events: serde_json::from_str(&events)?,
        resource: serde_json::from_str(&resource)?,
    })
}

fn compile_span_query(
    reader: &Reader,
    query: &Query,
) -> anyhow::Result<(WhereClause, Vec<String>)> {
    ensure_query_over_spans(query)?;
    let mut where_clause = WhereClause::within_reader_range(reader, "span.started_at");
    let aliases = TableAliases {
        record: "span",
        resource: "resource",
    };
    let unindexed = compile_query(
        query,
        aliases,
        reader.indexed_attributes(),
        "q",
        &mut where_clause,
    )?;
    Ok((where_clause, unindexed))
}

pub(super) fn explain_spans(reader: &Reader, query: &Query) -> anyhow::Result<Vec<String>> {
    let (where_clause, _) = compile_span_query(reader, query)?;
    reader.explain_scan(
        ["", " ORDER BY started_at DESC"],
        select_spans(&where_clause),
        &where_clause,
    )
}

fn ensure_query_over_spans(query: &Query) -> anyhow::Result<()> {
    ensure!(
        query.signal == Signal::Spans,
        "the query is over {}, not spans",
        query.signal
    );
    Ok(())
}

impl Reader {
    fn read_trace_stats(
        &self,
        trace_ids: impl Iterator<Item = TraceId>,
    ) -> anyhow::Result<HashMap<TraceId, TraceStats>> {
        let mut where_clause = WhereClause::new();
        let mut param_names = Vec::new();
        for (index, trace_id) in trace_ids.enumerate() {
            let name = format!(":t{index}");
            where_clause.push_param(&name, trace_id.0.to_vec());
            param_names.push(name);
        }
        if param_names.is_empty() {
            return Ok(HashMap::new());
        }
        let param_list = param_names.join(", ");
        let head = format!(
            "SELECT trace_id, count(*), max(status = {}) FROM (",
            SpanStatus::Error.number()
        );
        let rows = self.collect_rows(
            [&head, ") GROUP BY trace_id"],
            |day_schema| {
                format!(
                    "SELECT trace_id, status FROM {day_schema}.spans WHERE trace_id IN ({param_list})"
                )
            },
            &where_clause,
            |row| {
                Ok((
                    trace_id_from_blob(row.get(0)?)?,
                    TraceStats {
                        spans: u64::try_from(row.get::<_, i64>(1)?)?,
                        failed: row.get(2)?,
                    },
                ))
            },
        )?;
        Ok(rows.into_iter().collect())
    }
}

pub(super) fn read_spans(reader: &Reader, query: &Query, limit: usize) -> anyhow::Result<Spans> {
    let (where_clause, unindexed) = compile_span_query(reader, query)?;
    let tail = format!(" ORDER BY started_at DESC LIMIT {}", limit + 1);
    let mut spans = reader.collect_rows(
        ["", &tail],
        select_spans(&where_clause),
        &where_clause,
        trace_span_from_row,
    )?;
    let truncated = truncate_to_limit(&mut spans, limit);
    Ok(Spans {
        spans,
        truncated,
        unindexed,
    })
}

pub(super) fn read_traces(reader: &Reader, query: &Query, limit: usize) -> anyhow::Result<Traces> {
    ensure_query_over_spans(query)?;
    let mut where_clause = WhereClause::within_reader_range(reader, "span.started_at");
    where_clause.push_condition("span.parent_span_id IS NULL".into());
    let mut unindexed = Vec::new();
    if query.expression.is_some() {
        // A matching span may sit in another day file than its root.
        let mut matching = WhereClause::within_reader_range(reader, "matching_span.started_at");
        let aliases = TableAliases {
            record: "matching_span",
            resource: "matching_resource",
        };
        unindexed = compile_query(
            query,
            aliases,
            reader.indexed_attributes(),
            "q",
            &mut matching,
        )?;
        let matching_trace_ids_sql = union_day_selects(reader.days(), |day_schema| {
            format!(
                "SELECT matching_span.trace_id
                 FROM {day_schema}.spans matching_span
                 JOIN {day_schema}.resources matching_resource
                   ON matching_resource.id = matching_span.resource_id
                 WHERE {}",
                matching.sql_for_day(day_schema)
            )
        });
        where_clause.push_condition(format!("span.trace_id IN ({matching_trace_ids_sql})"));
        where_clause.absorb_params(matching);
    }
    let tail = format!(" ORDER BY started_at DESC LIMIT {}", limit + 1);
    let mut roots = reader.collect_rows(
        ["", &tail],
        |day_schema| {
            format!(
                "SELECT span.trace_id, span.started_at, resource.service, span.name, span.kind,
                        span.duration_ns, span.attributes,
                        resource.attributes AS resource_attributes
                 FROM {day_schema}.spans span
                 JOIN {day_schema}.resources resource ON resource.id = span.resource_id
                 WHERE {}",
                where_clause.sql_for_day(day_schema)
            )
        },
        &where_clause,
        |row| {
            let attributes: String = row.get(6)?;
            let resource: String = row.get(7)?;
            Ok(RootSpan {
                trace_id: trace_id_from_blob(row.get(0)?)?,
                started_at: row.get(1)?,
                service: row.get(2)?,
                name: row.get(3)?,
                kind: SpanKind::from_number(row.get(4)?),
                duration_ns: row.get(5)?,
                attributes: serde_json::from_str(&attributes)?,
                resource: serde_json::from_str(&resource)?,
            })
        },
    )?;
    let truncated = truncate_to_limit(&mut roots, limit);
    let stats = reader.read_trace_stats(roots.iter().map(|root| root.trace_id))?;
    let traces = roots
        .into_iter()
        .map(|root| {
            let stats = stats.get(&root.trace_id).copied().unwrap_or(TraceStats {
                spans: 1,
                failed: false,
            });
            TraceSummary {
                trace_id: root.trace_id,
                started_at: timestamp_from_nanos(root.started_at),
                service: root.service,
                name: root.name,
                kind: root.kind,
                duration_ns: root.duration_ns,
                spans: stats.spans,
                error: stats.failed,
                attributes: root.attributes,
                resource: root.resource,
            }
        })
        .collect();
    Ok(Traces {
        traces,
        truncated,
        unindexed,
    })
}

pub(super) fn read_trace(
    reader: &Reader,
    id: TraceId,
    limit: usize,
) -> anyhow::Result<Option<Trace>> {
    let mut where_clause = WhereClause::within_reader_range(reader, "span.started_at");
    where_clause.push_condition_with_param("span.trace_id = :trace_id", ":trace_id", id.0.to_vec());
    let tail = format!(" ORDER BY started_at LIMIT {}", limit + 1);
    let mut spans = reader.collect_rows(
        ["", &tail],
        select_spans(&where_clause),
        &where_clause,
        trace_span_from_row,
    )?;
    let spans_truncated = truncate_to_limit(&mut spans, limit);
    let trace_logs_query = Query {
        signal: Signal::Logs,
        expression: Some(Expression::Compare {
            field: Field::Builtin(BuiltinField::TraceId),
            operator: Operator::Eq,
            value: otelo_query::Value::String(id.to_string()),
        }),
    };
    let logs = super::logs::read_logs(reader, &trace_logs_query, limit)?;
    if spans.is_empty() && logs.logs.is_empty() {
        return Ok(None);
    }
    Ok(Some(Trace {
        trace_id: id,
        spans,
        logs: logs.logs,
        truncated: spans_truncated || logs.truncated,
    }))
}
