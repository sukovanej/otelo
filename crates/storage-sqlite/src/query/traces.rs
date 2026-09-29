use std::collections::HashMap;

use anyhow::ensure;
use rusqlite::Row;
use siner_query::{Builtin, Expr, Field, Op, Query, Signal};
use siner_storage::query::{Spans, Trace, TraceSpan, TraceSummary, Traces};
use siner_storage::{Attributes, SpanKind, SpanStatus, TraceId};

use super::compile::{TableAliases, compile_query};
use super::services::SpanLocation;
use super::{
    WhereClause, new_statement_span, span_id_from_blob, timestamp_from_nanos, trace_id_from_blob,
    truncate_to_limit, union_day_selects,
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

const SPAN_COLUMNS: &str = "s.trace_id, s.span_id, s.parent_span_id, r.service, s.name, s.kind,
    s.start_ts, s.duration_ns, s.status, s.attributes, s.events, r.attributes AS resource";

fn select_spans(where_: &WhereClause) -> impl Fn(&str) -> String {
    move |day| {
        format!(
            "SELECT {SPAN_COLUMNS}
             FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
             WHERE {}",
            where_.sql_for_day(day)
        )
    }
}

impl Reader {
    pub(super) fn read_spans_at(
        &self,
        locations: &[SpanLocation],
    ) -> anyhow::Result<Vec<TraceSpan>> {
        let mut rowids_by_day: HashMap<&str, Vec<String>> = HashMap::new();
        for (day, rowid) in locations {
            rowids_by_day
                .entry(day)
                .or_default()
                .push(rowid.to_string());
        }
        let mut spans = Vec::with_capacity(locations.len());
        for (day, rowids) in rowids_by_day {
            let sql = format!(
                "SELECT {SPAN_COLUMNS}
                 FROM \"{day}\".spans s JOIN \"{day}\".resources r ON r.id = s.resource_id
                 WHERE s.rowid IN ({})",
                rowids.join(", ")
            );
            let statement = new_statement_span(&sql);
            let _entered = statement.enter();
            let mut stmt = self.conn().prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut read = 0_i64;
            while let Some(row) = rows.next()? {
                read += 1;
                spans.push(span(row)?);
            }
            statement.record("db.response.returned_rows", read);
        }
        spans.sort_by_key(|span| std::cmp::Reverse(span.started_at));
        Ok(spans)
    }
}

fn span(row: &Row) -> anyhow::Result<TraceSpan> {
    let parent: Option<Vec<u8>> = row.get(2)?;
    let attributes: String = row.get(9)?;
    let events: String = row.get(10)?;
    let resource: String = row.get(11)?;
    Ok(TraceSpan {
        trace_id: trace_id_from_blob(row.get(0)?)?,
        span_id: span_id_from_blob(row.get(1)?)?,
        parent_span_id: parent.map(span_id_from_blob).transpose()?,
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
    check(query)?;
    let mut where_ = WhereClause::within_reader_range(reader, "s.start_ts");
    let aliases = TableAliases {
        record: "s",
        resource: "r",
    };
    let unindexed = compile_query(
        query,
        aliases,
        reader.indexed_attributes(),
        "q",
        &mut where_,
    )?;
    Ok((where_, unindexed))
}

pub(super) fn explain_spans(reader: &Reader, query: &Query) -> anyhow::Result<Vec<String>> {
    let (where_, _) = compile_span_query(reader, query)?;
    reader.explain_scan(
        ["", " ORDER BY start_ts DESC"],
        select_spans(&where_),
        &where_,
    )
}

fn check(query: &Query) -> anyhow::Result<()> {
    ensure!(
        query.signal == Signal::Spans,
        "the query is over {}, not spans",
        query.signal
    );
    Ok(())
}

impl Reader {
    fn trace_stats(
        &self,
        ids: impl Iterator<Item = TraceId>,
    ) -> anyhow::Result<HashMap<TraceId, TraceStats>> {
        let mut where_ = WhereClause::new();
        let mut names = Vec::new();
        for (i, id) in ids.enumerate() {
            let name = format!(":t{i}");
            where_.push_param(&name, id.0.to_vec());
            names.push(name);
        }
        if names.is_empty() {
            return Ok(HashMap::new());
        }
        let list = names.join(", ");
        let head = format!(
            "SELECT trace_id, count(*), max(status = {}) FROM (",
            SpanStatus::Error.number()
        );
        let rows = self.collect_rows(
            [&head, ") GROUP BY trace_id"],
            |day| format!("SELECT trace_id, status FROM {day}.spans WHERE trace_id IN ({list})"),
            &where_,
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
    let (where_, unindexed) = compile_span_query(reader, query)?;
    let tail = format!(" ORDER BY start_ts DESC LIMIT {}", limit + 1);
    let mut spans = reader.collect_rows(["", &tail], select_spans(&where_), &where_, span)?;
    let truncated = truncate_to_limit(&mut spans, limit);
    Ok(Spans {
        spans,
        truncated,
        unindexed,
    })
}

pub(super) fn read_traces(reader: &Reader, query: &Query, limit: usize) -> anyhow::Result<Traces> {
    check(query)?;
    let mut where_ = WhereClause::within_reader_range(reader, "s.start_ts");
    where_.push_clause("s.parent_span_id IS NULL".into());
    let mut unindexed = Vec::new();
    if query.expr.is_some() {
        // A matching span may sit in another day file than its root.
        let mut matching = WhereClause::within_reader_range(reader, "m.start_ts");
        let aliases = TableAliases {
            record: "m",
            resource: "mr",
        };
        unindexed = compile_query(
            query,
            aliases,
            reader.indexed_attributes(),
            "q",
            &mut matching,
        )?;
        let ids = union_day_selects(reader.days(), |day| {
            format!(
                "SELECT m.trace_id FROM {day}.spans m
                 JOIN {day}.resources mr ON mr.id = m.resource_id
                 WHERE {}",
                matching.sql_for_day(day)
            )
        });
        where_.push_clause(format!("s.trace_id IN ({ids})"));
        where_.absorb_params(matching);
    }
    let tail = format!(" ORDER BY start_ts DESC LIMIT {}", limit + 1);
    let mut roots = reader.collect_rows(
        ["", &tail],
        |day| {
            format!(
                "SELECT s.trace_id, s.start_ts, r.service, s.name, s.kind, s.duration_ns,
                    s.attributes, r.attributes AS resource
                 FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
                 WHERE {}",
                where_.sql_for_day(day)
            )
        },
        &where_,
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
    let stats = reader.trace_stats(roots.iter().map(|root| root.trace_id))?;
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
    let mut where_ = WhereClause::within_reader_range(reader, "s.start_ts");
    where_.push_clause_with_param("s.trace_id = :trace_id", ":trace_id", id.0.to_vec());
    let tail = format!(" ORDER BY start_ts LIMIT {}", limit + 1);
    let mut spans = reader.collect_rows(["", &tail], select_spans(&where_), &where_, span)?;
    let spans_cut = truncate_to_limit(&mut spans, limit);
    let logs = Query {
        signal: Signal::Logs,
        expr: Some(Expr::Compare {
            field: Field::Builtin(Builtin::TraceId),
            op: Op::Eq,
            value: siner_query::Value::String(id.to_string()),
        }),
    };
    let logs = super::logs::read_logs(reader, &logs, limit)?;
    if spans.is_empty() && logs.logs.is_empty() {
        return Ok(None);
    }
    Ok(Some(Trace {
        trace_id: id,
        spans,
        logs: logs.logs,
        truncated: spans_cut || logs.truncated,
    }))
}
