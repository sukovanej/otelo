use std::collections::HashMap;

use anyhow::ensure;
use otelo_indexed_storage::query::{SpanSort, Spans, Trace, TraceSpan, TraceSummary, Traces};
use otelo_indexed_storage::{Attributes, SpanKind, SpanStatus, TraceId};
use otelo_query::{BuiltinField, Expression, Field, Operator, Query, Signal};
use rusqlite::Row;

use super::compile::{TableAliases, compile_query};
use super::{
    WhereClause, row_limit_with_one_more, span_id_from_blob, timestamp_from_nanos,
    trace_id_from_blob, truncate_to_limit,
};
use crate::Reader;

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
    span.name, span.kind, span.started_at, span.duration_ns, span.status_code, span.attributes,
    span.events, resource.attributes AS resource_attributes";

fn select_spans(where_clause: &WhereClause) -> String {
    format!(
        "SELECT {SPAN_COLUMNS}
         FROM spans span
         JOIN resources resource ON resource.id = span.resource_id
         WHERE {}",
        where_clause.sql()
    )
}

const fn pick_order_columns(sort: SpanSort) -> &'static str {
    match sort {
        SpanSort::Newest => "span.started_at DESC",
        SpanSort::Oldest => "span.started_at",
        SpanSort::Longest => "span.duration_ns DESC, span.started_at DESC",
        SpanSort::Shortest => "span.duration_ns, span.started_at DESC",
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

pub(super) fn compile_span_query(
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
        &format!(
            "{} ORDER BY span.started_at DESC",
            select_spans(&where_clause)
        ),
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
        let trace_ids: Vec<String> = trace_ids.map(|trace_id| trace_id.to_string()).collect();
        if trace_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut where_clause = WhereClause::new();
        where_clause.push_param(":trace_ids", serde_json::to_string(&trace_ids)?);
        let sql = format!(
            "SELECT trace_id, count(*), max(status_code = {})
             FROM spans
             WHERE trace_id IN (SELECT unhex(value) FROM json_each(:trace_ids))
             GROUP BY trace_id",
            SpanStatus::Error.number()
        );
        let rows = self.collect_rows(&sql, &where_clause, |row| {
            Ok((
                trace_id_from_blob(row.get(0)?)?,
                TraceStats {
                    spans: u64::try_from(row.get::<_, i64>(1)?)?,
                    failed: row.get(2)?,
                },
            ))
        })?;
        Ok(rows.into_iter().collect())
    }
}

pub(super) fn read_spans(
    reader: &Reader,
    query: &Query,
    sort: SpanSort,
    limit: usize,
) -> anyhow::Result<Spans> {
    let (mut where_clause, unindexed) = compile_span_query(reader, query)?;
    where_clause.push_param(":limit", row_limit_with_one_more(limit)?);
    let sql = format!(
        "{} ORDER BY {} LIMIT :limit",
        select_spans(&where_clause),
        pick_order_columns(sort)
    );
    let mut spans = reader.collect_rows(&sql, &where_clause, trace_span_from_row)?;
    let truncated = truncate_to_limit(&mut spans, limit);
    Ok(Spans {
        spans,
        truncated,
        unindexed,
    })
}

pub(super) fn read_traces(
    reader: &Reader,
    query: &Query,
    sort: SpanSort,
    limit: usize,
) -> anyhow::Result<Traces> {
    ensure_query_over_spans(query)?;
    let mut where_clause = WhereClause::within_reader_range(reader, "span.started_at");
    where_clause.push_condition("span.parent_span_id IS NULL".into());
    let mut unindexed = Vec::new();
    if query.expression.is_some() {
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
        where_clause.push_condition(format!(
            "span.trace_id IN (SELECT matching_span.trace_id
                               FROM spans matching_span
                               JOIN resources matching_resource
                                 ON matching_resource.id = matching_span.resource_id
                               WHERE {})",
            matching.sql()
        ));
        where_clause.absorb_params(matching);
    }
    let sql = format!(
        "SELECT span.trace_id, span.started_at, resource.service, span.name, span.kind,
                span.duration_ns, span.attributes, resource.attributes AS resource_attributes
         FROM spans span
         JOIN resources resource ON resource.id = span.resource_id
         WHERE {}
         ORDER BY {}
         LIMIT :limit",
        where_clause.sql(),
        pick_order_columns(sort)
    );
    where_clause.push_param(":limit", row_limit_with_one_more(limit)?);
    let mut roots = reader.collect_rows(&sql, &where_clause, |row| {
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
    })?;
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
    where_clause.push_param(":limit", row_limit_with_one_more(limit)?);
    let sql = format!(
        "{} ORDER BY span.started_at LIMIT :limit",
        select_spans(&where_clause)
    );
    let mut spans = reader.collect_rows(&sql, &where_clause, trace_span_from_row)?;
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
