use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;

use super::{Filter, LogFilter, LogLine, STATUS_ERROR, cut, hex, time, union};
use crate::Reader;

#[derive(Clone, Debug, Default)]
pub struct TraceFilter {
    /// The service of the root span.
    pub service: Option<String>,
    /// Text that the name of the root span contains.
    pub name: Option<String>,
    pub min_duration_ns: Option<i64>,
    /// Keep only the traces with a failed span.
    pub errors: bool,
}

/// Traces by their root span, newest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Traces {
    pub traces: Vec<TraceSummary>,
    /// More traces match than the limit let through.
    pub truncated: bool,
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
}

impl Reader {
    /// The newest `limit` traces whose root span passes `filter`.
    ///
    /// # Errors
    ///
    /// When the query fails, such as when it runs past the time limit.
    pub fn traces(&self, filter: &TraceFilter, limit: usize) -> anyhow::Result<Traces> {
        let mut where_ = Filter::range(self, "s.start_ts");
        where_.push_clause("s.parent_span_id IS NULL".into());
        if let Some(service) = &filter.service {
            where_.push("r.service = :service", ":service", service.clone());
        }
        if let Some(name) = &filter.name {
            where_.push("instr(s.name, :name) > 0", ":name", name.clone());
        }
        if let Some(min) = filter.min_duration_ns {
            where_.push("s.duration_ns >= :min_duration", ":min_duration", min);
        }
        if filter.errors {
            // A failed span can be in another day file than its root.
            let failed = union(self.days(), |day| {
                format!(
                    "SELECT 1 FROM {day}.spans e
                     WHERE e.trace_id = s.trace_id AND e.status = {STATUS_ERROR}"
                )
            });
            where_.push_clause(format!("EXISTS ({failed})"));
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
        Ok(Traces { traces, truncated })
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
        let mut spans = self.collect(
            ["", &tail],
            |day| {
                format!(
                    "SELECT s.span_id, s.parent_span_id, r.service, s.name, s.kind, s.start_ts,
                            s.duration_ns, s.status, s.attributes, s.events
                     FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    where_.sql(day)
                )
            },
            &where_,
            |row| {
                let parent: Option<Vec<u8>> = row.get(1)?;
                let status: i32 = row.get(7)?;
                let attributes: String = row.get(8)?;
                let events: String = row.get(9)?;
                Ok(TraceSpan {
                    span_id: hex(&row.get::<_, Vec<u8>>(0)?),
                    parent_span_id: parent.as_deref().map(hex),
                    service: row.get(2)?,
                    name: row.get(3)?,
                    kind: row.get(4)?,
                    time: time(row.get(5)?),
                    duration_ns: row.get(6)?,
                    status,
                    error: status == STATUS_ERROR,
                    attributes: serde_json::from_str(&attributes)?,
                    events: serde_json::from_str(&events)?,
                })
            },
        )?;
        let spans_cut = cut(&mut spans, limit);
        let logs = self.logs(
            &LogFilter {
                trace_id: Some(id),
                ..LogFilter::default()
            },
            limit,
        )?;
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
