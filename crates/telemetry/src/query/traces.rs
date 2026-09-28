use std::collections::HashMap;

use anyhow::ensure;
use jiff::Timestamp;
use rusqlite::Row;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use siner_query::{Builtin, Expr, Field, Op, Query, Signal};
use utoipa::ToSchema;

use super::compile::{Aliases, compile};
use super::{Filter, LogLine, STATUS_ERROR, cut, hex, time, union};
use crate::Reader;

/// Traces by their root span, newest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Traces {
    pub traces: Vec<TraceSummary>,
    /// More traces match than the limit let through.
    pub truncated: bool,
    /// The attributes the query compares that have no index.
    pub unindexed: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TraceSummary {
    pub trace_id: String,
    /// The start of the root span.
    #[schema(value_type = String, format = DateTime)]
    pub time: Timestamp,
    pub service: String,
    /// The name of the root span.
    pub name: String,
    pub duration_ns: i64,
    pub spans: u64,
    /// Whether a span of the trace failed.
    pub error: bool,
}

/// Spans, newest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Spans {
    pub spans: Vec<TraceSpan>,
    /// More spans match than the limit let through.
    pub truncated: bool,
    /// The attributes the query compares that have no index.
    pub unindexed: Vec<String>,
}

/// One trace: its spans by start time, and the logs that carry its ID.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Trace {
    pub trace_id: String,
    pub spans: Vec<TraceSpan>,
    /// Newest first.
    pub logs: Vec<LogLine>,
    /// The trace has more spans or logs than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub service: String,
    pub name: String,
    /// The OpenTelemetry span kind.
    pub kind: i32,
    #[schema(value_type = String, format = DateTime)]
    pub time: Timestamp,
    pub duration_ns: i64,
    /// The OpenTelemetry status code.
    pub status: i32,
    pub error: bool,
    #[schema(value_type = Object)]
    pub attributes: Map<String, Value>,
    pub events: Vec<Value>,
    /// The attributes of the resource that sent the span.
    #[schema(value_type = Object)]
    pub resource: Map<String, Value>,
}

const SPAN_COLUMNS: &str = "s.trace_id, s.span_id, s.parent_span_id, r.service, s.name, s.kind,
    s.start_ts, s.duration_ns, s.status, s.attributes, s.events, r.attributes AS resource";

fn select_spans(where_: &Filter) -> impl Fn(&str) -> String {
    move |day| {
        format!(
            "SELECT {SPAN_COLUMNS}
             FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
             WHERE {}",
            where_.sql(day)
        )
    }
}

fn span(row: &Row) -> anyhow::Result<TraceSpan> {
    let parent: Option<Vec<u8>> = row.get(2)?;
    let status: i32 = row.get(8)?;
    let attributes: String = row.get(9)?;
    let events: String = row.get(10)?;
    let resource: String = row.get(11)?;
    Ok(TraceSpan {
        trace_id: hex(&row.get::<_, Vec<u8>>(0)?),
        span_id: hex(&row.get::<_, Vec<u8>>(1)?),
        parent_span_id: parent.as_deref().map(hex),
        service: row.get(3)?,
        name: row.get(4)?,
        kind: row.get(5)?,
        time: time(row.get(6)?),
        duration_ns: row.get(7)?,
        status,
        error: status == STATUS_ERROR,
        attributes: serde_json::from_str(&attributes)?,
        events: serde_json::from_str(&events)?,
        resource: serde_json::from_str(&resource)?,
    })
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
    /// The newest `limit` spans in the range that `query` keeps.
    ///
    /// # Errors
    ///
    /// When the query is invalid for spans, or when it fails, such as when it
    /// runs past the time limit.
    pub fn spans(&self, query: &Query, limit: usize) -> anyhow::Result<Spans> {
        check(query)?;
        let mut where_ = Filter::range(self, "s.start_ts");
        let aliases = Aliases {
            record: "s",
            resource: "r",
        };
        let unindexed = compile(query, aliases, self.indexes(), "q", &mut where_)?;
        let tail = format!(" ORDER BY start_ts DESC LIMIT {}", limit + 1);
        let mut spans = self.collect(["", &tail], select_spans(&where_), &where_, span)?;
        let truncated = cut(&mut spans, limit);
        Ok(Spans {
            spans,
            truncated,
            unindexed,
        })
    }

    /// The newest `limit` traces with a span in the range that `query` keeps,
    /// by their root span.
    ///
    /// # Errors
    ///
    /// When the query is invalid for spans, or when it fails, such as when it
    /// runs past the time limit.
    pub fn traces(&self, query: &Query, limit: usize) -> anyhow::Result<Traces> {
        check(query)?;
        let mut where_ = Filter::range(self, "s.start_ts");
        where_.push_clause("s.parent_span_id IS NULL".into());
        let mut unindexed = Vec::new();
        if query.expr.is_some() {
            // The traces that have a matching span, from any day file.
            let mut matching = Filter::range(self, "m.start_ts");
            let aliases = Aliases {
                record: "m",
                resource: "mr",
            };
            unindexed = compile(query, aliases, self.indexes(), "q", &mut matching)?;
            let ids = union(self.days(), |day| {
                format!(
                    "SELECT m.trace_id FROM {day}.spans m
                     JOIN {day}.resources mr ON mr.id = m.resource_id
                     WHERE {}",
                    matching.sql(day)
                )
            });
            where_.push_clause(format!("s.trace_id IN ({ids})"));
            where_.absorb(matching);
        }
        let tail = format!(" ORDER BY start_ts DESC LIMIT {}", limit + 1);
        let mut roots = self.collect(
            ["", &tail],
            |day| {
                format!(
                    "SELECT s.trace_id, s.start_ts, r.service, s.name, s.duration_ns
                     FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    where_.sql(day)
                )
            },
            &where_,
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )?;
        let truncated = cut(&mut roots, limit);
        let stats = self.span_stats(roots.iter().map(|root| root.0.as_slice()))?;
        let traces = roots
            .into_iter()
            .map(|(id, start, service, name, duration_ns)| {
                let (spans, error) = stats.get(&id).copied().unwrap_or((1, false));
                TraceSummary {
                    trace_id: hex(&id),
                    time: time(start),
                    service,
                    name,
                    duration_ns,
                    spans,
                    error,
                }
            })
            .collect();
        Ok(Traces {
            traces,
            truncated,
            unindexed,
        })
    }

    /// The span count and the error flag of each trace, over every attached
    /// day.
    fn span_stats<'a>(
        &self,
        ids: impl Iterator<Item = &'a [u8]>,
    ) -> anyhow::Result<HashMap<Vec<u8>, (u64, bool)>> {
        let mut where_ = Filter::new();
        let mut names = Vec::new();
        for (i, id) in ids.enumerate() {
            let name = format!(":t{i}");
            where_.param(&name, id.to_vec());
            names.push(name);
        }
        if names.is_empty() {
            return Ok(HashMap::new());
        }
        let list = names.join(", ");
        let head = format!("SELECT trace_id, count(*), max(status = {STATUS_ERROR}) FROM (");
        let rows = self.collect(
            [&head, ") GROUP BY trace_id"],
            |day| format!("SELECT trace_id, status FROM {day}.spans WHERE trace_id IN ({list})"),
            &where_,
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    (
                        u64::try_from(row.get::<_, i64>(1)?)?,
                        row.get::<_, bool>(2)?,
                    ),
                ))
            },
        )?;
        Ok(rows.into_iter().collect())
    }

    /// The spans of the trace `id` and the logs that carry its ID, `limit` of
    /// each at most. `None` when the range has neither.
    ///
    /// # Errors
    ///
    /// When the query fails, such as when it runs past the time limit.
    pub fn trace(&self, id: [u8; 16], limit: usize) -> anyhow::Result<Option<Trace>> {
        let mut where_ = Filter::range(self, "s.start_ts");
        where_.push("s.trace_id = :trace_id", ":trace_id", id.to_vec());
        let tail = format!(" ORDER BY start_ts LIMIT {}", limit + 1);
        let mut spans = self.collect(["", &tail], select_spans(&where_), &where_, span)?;
        let spans_cut = cut(&mut spans, limit);
        let logs = Query {
            signal: Signal::Logs,
            expr: Some(Expr::Compare {
                field: Field::Builtin(Builtin::TraceId),
                op: Op::Eq,
                value: siner_query::Value::String(hex(&id)),
            }),
        };
        let logs = self.logs(&logs, limit)?;
        if spans.is_empty() && logs.logs.is_empty() {
            return Ok(None);
        }
        Ok(Some(Trace {
            trace_id: hex(&id),
            spans,
            logs: logs.logs,
            truncated: spans_cut || logs.truncated,
        }))
    }
}
