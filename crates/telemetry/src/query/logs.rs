use std::collections::{BTreeSet, HashMap};

use jiff::Timestamp;
use rusqlite::Row;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::ToSchema;

use super::{Filter, cut, hex, level, template, time};
use crate::Reader;

/// The most lines [`Reader::log_groups`] reads, newest first. A range with
/// more lines groups only these.
pub const GROUP_SCAN_LIMIT: u64 = 50_000;

/// How many different bodies a group keeps as samples.
const SAMPLES: usize = 3;

#[derive(Clone, Debug, Default)]
pub struct LogFilter {
    pub service: Option<String>,
    /// The lowest severity number to keep.
    pub min_severity: Option<i32>,
    /// Words that every body has to contain, found with FTS5.
    pub search: Option<String>,
    pub trace_id: Option<[u8; 16]>,
}

/// Log lines, newest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Logs {
    pub logs: Vec<LogLine>,
    /// More lines match than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogLine {
    #[schema(value_type = String, format = DateTime)]
    pub time: Timestamp,
    pub service: String,
    /// The OpenTelemetry severity number.
    pub severity: i32,
    /// The name of the severity: TRACE, DEBUG, INFO, WARN, ERROR, or FATAL.
    pub level: String,
    pub body: String,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    #[schema(value_type = Object)]
    pub attributes: Map<String, Value>,
    /// `otlp`, or the service log source that read the line.
    pub source: String,
}

/// Log lines grouped by message template, the largest group first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogGroups {
    pub groups: Vec<LogGroup>,
    /// More groups exist than the limit let through.
    pub truncated: bool,
    /// How many lines the groups count.
    pub scanned: u64,
    /// The range has more lines than the grouping reads, so the groups count
    /// only the newest ones.
    pub partial: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct LogGroup {
    /// The body with numbers, UUIDs, hex IDs, and quoted strings replaced by
    /// placeholders.
    pub template: String,
    pub count: u64,
    /// The highest severity number in the group.
    pub severity: i32,
    pub level: String,
    pub services: Vec<String>,
    #[schema(value_type = String, format = DateTime)]
    pub first: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub last: Timestamp,
    /// Up to three different bodies, newest first.
    pub samples: Vec<String>,
}

impl LogFilter {
    fn filter(&self, reader: &Reader) -> Filter {
        let mut filter = Filter::range(reader, "l.ts");
        if let Some(service) = &self.service {
            filter.push("r.service = :service", ":service", service.clone());
        }
        if let Some(severity) = self.min_severity {
            filter.push("l.severity >= :severity", ":severity", i64::from(severity));
        }
        if let Some(id) = self.trace_id {
            filter.push("l.trace_id = :trace_id", ":trace_id", id.to_vec());
        }
        if let Some(search) = self.search.as_deref().and_then(fts_query) {
            filter.push(
                "l.rowid IN (SELECT rowid FROM $day.logs_fts WHERE logs_fts MATCH :search)",
                ":search",
                search,
            );
        }
        filter
    }
}

/// Each word of `text` as an FTS5 string, so that punctuation in it is
/// matched and not read as query syntax. `None` when there are no words.
fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

fn select(columns: &str, filter: &Filter) -> impl Fn(&str) -> String {
    move |day| {
        format!(
            "SELECT {columns} FROM {day}.logs l JOIN {day}.resources r ON r.id = l.resource_id
             WHERE {}",
            filter.sql(day)
        )
    }
}

impl Reader {
    /// The newest `limit` log lines that pass `filter`.
    ///
    /// # Errors
    ///
    /// When the query fails, such as when it runs past the time limit.
    pub fn logs(&self, filter: &LogFilter, limit: usize) -> anyhow::Result<Logs> {
        let where_ = filter.filter(self);
        let columns =
            "l.ts, r.service, l.severity, l.body, l.trace_id, l.span_id, l.attributes, l.source";
        let tail = format!(" ORDER BY ts DESC LIMIT {}", limit + 1);
        let mut logs = self.collect(["", &tail], select(columns, &where_), &where_, log_line)?;
        let truncated = cut(&mut logs, limit);
        Ok(Logs { logs, truncated })
    }

    /// The log lines that pass `filter`, grouped by message template. Reads
    /// the newest [`GROUP_SCAN_LIMIT`] lines at most, and returns the `limit`
    /// largest groups.
    ///
    /// # Errors
    ///
    /// When the query fails, such as when it runs past the time limit.
    pub fn log_groups(&self, filter: &LogFilter, limit: usize) -> anyhow::Result<LogGroups> {
        let where_ = filter.filter(self);
        let columns = "l.ts, r.service, l.severity, l.body";
        let tail = format!(" ORDER BY ts DESC LIMIT {}", GROUP_SCAN_LIMIT + 1);
        let mut groups: HashMap<String, Acc> = HashMap::new();
        let mut scanned = 0;
        let mut partial = false;
        self.scan(["", &tail], select(columns, &where_), &where_, |row| {
            if scanned == GROUP_SCAN_LIMIT {
                partial = true;
                return Ok(false);
            }
            scanned += 1;
            let ts: i64 = row.get(0)?;
            let service: String = row.get(1)?;
            let severity: i32 = row.get(2)?;
            let body: String = row.get(3)?;
            let acc = groups.entry(template(&body)).or_insert_with(|| Acc {
                count: 0,
                severity,
                services: BTreeSet::new(),
                first: ts,
                last: ts,
                samples: Vec::new(),
            });
            acc.count += 1;
            acc.severity = acc.severity.max(severity);
            acc.services.insert(service);
            // Rows come newest first.
            acc.first = ts;
            if acc.samples.len() < SAMPLES && !acc.samples.contains(&body) {
                acc.samples.push(body);
            }
            Ok(true)
        })?;
        let mut groups: Vec<LogGroup> = groups
            .into_iter()
            .map(|(template, acc)| LogGroup {
                template,
                count: acc.count,
                severity: acc.severity,
                level: level(acc.severity).into(),
                services: acc.services.into_iter().collect(),
                first: time(acc.first),
                last: time(acc.last),
                samples: acc.samples,
            })
            .collect();
        groups.sort_by(|a, b| (b.count, b.last, &a.template).cmp(&(a.count, a.last, &b.template)));
        let truncated = cut(&mut groups, limit);
        Ok(LogGroups {
            groups,
            truncated,
            scanned,
            partial,
        })
    }
}

struct Acc {
    count: u64,
    severity: i32,
    services: BTreeSet<String>,
    first: i64,
    last: i64,
    samples: Vec<String>,
}

fn log_line(row: &Row) -> anyhow::Result<LogLine> {
    let severity: i32 = row.get(2)?;
    let trace_id: Option<Vec<u8>> = row.get(4)?;
    let span_id: Option<Vec<u8>> = row.get(5)?;
    let attributes: String = row.get(6)?;
    Ok(LogLine {
        time: time(row.get(0)?),
        service: row.get(1)?,
        severity,
        level: level(severity).into(),
        body: row.get(3)?,
        trace_id: trace_id.as_deref().map(hex),
        span_id: span_id.as_deref().map(hex),
        attributes: serde_json::from_str(&attributes)?,
        source: row.get(7)?,
    })
}
