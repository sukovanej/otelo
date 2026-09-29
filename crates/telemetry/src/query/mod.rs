//! The queries behind the HTTP API: logs, traces, metrics, services, calls,
//! and SQL over the day files a [`Reader`] attached.
//!
//! A query that filters reads each day file on its own and joins the parts
//! with `UNION ALL`, because FTS5 and the ids only work within one file. Every
//! query takes a row limit and says when it cut the result.

mod calls;
mod catalog;
mod compile;
mod logs;
mod metrics;
mod services;
mod sql;
mod template;
mod traces;

use anyhow::{Context, bail};
use jiff::Timestamp;
use rusqlite::types::Value;
use rusqlite::{Row, ToSql};

pub use calls::{
    CallDetail, CallOperation, Calls, Target, TargetKey, TargetType, path_template, query_template,
};
pub use catalog::{Attribute, AttributeKeys, ReaderCatalog};
pub use compile::InvalidQuery;
pub use logs::{GROUP_SCAN_LIMIT, LogGroup, LogGroups, LogLine, Logs};
pub use metrics::{
    Bucket, MAX_BUCKETS, MetricFilter, MetricList, MetricSeries, Series, SeriesInfo, default_step,
    step_for,
};
pub use services::{
    Latency, Operation, OperationDetail, RequestBucket, Requests, Service, ServiceBucket,
    ServiceStats, ServiceSummary, Services,
};
pub use sql::{SqlResult, SqlValue};
pub use template::template;
pub use traces::{Spans, Trace, TraceSpan, TraceSummary, Traces};

use crate::{Day, Reader};
use siner_query::{Query, Signal};

/// The OpenTelemetry status code of a failed span.
pub const STATUS_ERROR: i32 = 2;

/// The name of an OpenTelemetry severity number.
#[must_use]
pub const fn level(severity: i32) -> &'static str {
    match severity {
        1..=4 => "TRACE",
        5..=8 => "DEBUG",
        9..=12 => "INFO",
        13..=16 => "WARN",
        17..=20 => "ERROR",
        21..=24 => "FATAL",
        _ => "UNSPECIFIED",
    }
}

/// The lowest severity number of a level name, or the number itself.
///
/// # Errors
///
/// When `text` is neither a level name nor a number.
pub fn parse_severity(text: &str) -> anyhow::Result<i32> {
    Ok(match text.to_ascii_lowercase().as_str() {
        "trace" => 1,
        "debug" => 5,
        "info" => 9,
        "warn" | "warning" => 13,
        "error" => 17,
        "fatal" => 21,
        number => number.parse().with_context(|| {
            format!(
                "{text:?} is not a severity: trace, debug, info, warn, error, fatal, or a number"
            )
        })?,
    })
}

/// A trace ID from its 32 hex digits.
///
/// # Errors
///
/// When `text` is not 32 hex digits.
pub fn parse_trace_id(text: &str) -> anyhow::Result<[u8; 16]> {
    let mut id = [0; 16];
    if text.len() != 32 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("{text:?} is not a trace ID of 32 hex digits");
    }
    for (i, byte) in id.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)?;
    }
    Ok(id)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

pub(crate) fn time(ts: i64) -> Timestamp {
    Timestamp::from_nanosecond(i128::from(ts)).expect("an i64 of nanoseconds is a valid timestamp")
}

/// The conditions of a `WHERE` clause and the named parameters they use. Each
/// day's part of a query uses the same clause with the same table aliases.
pub(crate) struct Filter {
    clauses: Vec<String>,
    params: Vec<(String, Value)>,
}

impl Filter {
    pub(crate) const fn new() -> Self {
        Self {
            clauses: Vec::new(),
            params: Vec::new(),
        }
    }

    /// Keeps the rows whose `ts` column falls in the range of the reader.
    pub(crate) fn range(reader: &Reader, ts: &str) -> Self {
        let mut filter = Self::new();
        filter.push(&format!("{ts} >= :since"), ":since", reader.since());
        filter.push(&format!("{ts} < :until"), ":until", reader.until());
        filter
    }

    /// Adds a parameter that a clause of its own does not read.
    pub(crate) fn param(&mut self, name: &str, value: impl Into<Value>) {
        self.params.push((name.to_owned(), value.into()));
    }

    /// Adds `clause`, which reads the parameter `name`.
    pub(crate) fn push(&mut self, clause: &str, name: &str, value: impl Into<Value>) {
        self.clauses.push(clause.to_owned());
        self.params.push((name.to_owned(), value.into()));
    }

    /// Takes the parameters of `other`, whose clauses went into one of this
    /// filter's.
    pub(crate) fn absorb(&mut self, other: Self) {
        for (name, value) in other.params {
            if !self.params.iter().any(|(n, _)| *n == name) {
                self.params.push((name, value));
            }
        }
    }

    /// Adds a clause without a parameter. `$day` in it stands for the
    /// quoted schema name of the day.
    pub(crate) fn push_clause(&mut self, clause: String) {
        self.clauses.push(clause);
    }

    /// The conditions for the day whose quoted schema name is `day`.
    pub(crate) fn sql(&self, day: &str) -> String {
        if self.clauses.is_empty() {
            return "TRUE".into();
        }
        self.clauses.join(" AND ").replace("$day", day)
    }

    fn params(&self) -> Vec<(&str, &dyn ToSql)> {
        self.params
            .iter()
            .map(|(name, value)| (name.as_str(), value as &dyn ToSql))
            .collect()
    }
}

impl Reader {
    /// Runs one `SELECT` per attached day, joined with `UNION ALL` between
    /// `head` and `tail`, and calls `each` on every row until it returns
    /// `false`. `part` writes the `SELECT` of one day from its quoted schema
    /// name.
    pub(crate) fn scan(
        &self,
        [head, tail]: [&str; 2],
        part: impl Fn(&str) -> String,
        filter: &Filter,
        mut each: impl FnMut(&Row) -> anyhow::Result<bool>,
    ) -> anyhow::Result<()> {
        if self.days().is_empty() {
            return Ok(());
        }
        let sql = format!("{head}{}{tail}", union(self.days(), part));
        let span = statement_span(&sql);
        let _entered = span.enter();
        let mut stmt = self.conn().prepare(&sql)?;
        let mut rows = stmt.query(filter.params().as_slice())?;
        let mut read = 0_i64;
        while let Some(row) = rows.next()? {
            read += 1;
            if !each(row)? {
                break;
            }
        }
        span.record("db.response.returned_rows", read);
        Ok(())
    }

    /// The steps of `EXPLAIN QUERY PLAN` for the query that
    /// [`Reader::scan`] runs with the same arguments.
    pub(crate) fn plan(
        &self,
        [head, tail]: [&str; 2],
        part: impl Fn(&str) -> String,
        filter: &Filter,
    ) -> anyhow::Result<Vec<String>> {
        if self.days().is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "EXPLAIN QUERY PLAN {head}{}{tail}",
            union(self.days(), part)
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let steps = stmt.query_map(filter.params().as_slice(), |row| row.get::<_, String>(3))?;
        Ok(steps.collect::<Result<_, _>>()?)
    }

    /// How SQLite runs the query of [`Reader::logs`] or [`Reader::spans`]
    /// for `query`: one line per step of `EXPLAIN QUERY PLAN`. A step that
    /// names an `attr_` index reads an indexed attribute.
    ///
    /// # Errors
    ///
    /// When the query is over metrics or is invalid, or SQLite cannot plan
    /// it.
    pub fn explain(&self, query: &Query) -> anyhow::Result<Vec<String>> {
        match query.signal {
            Signal::Logs => logs::explain(self, query),
            Signal::Spans => traces::explain(self, query),
            Signal::Metrics => bail!("a plan shows how logs and spans use their indexes"),
        }
    }

    /// Like [`Reader::scan`], and collects the rows `map` makes.
    pub(crate) fn collect<T>(
        &self,
        around: [&str; 2],
        part: impl Fn(&str) -> String,
        filter: &Filter,
        mut map: impl FnMut(&Row) -> anyhow::Result<T>,
    ) -> anyhow::Result<Vec<T>> {
        let mut out = Vec::new();
        self.scan(around, part, filter, |row| {
            out.push(map(row)?);
            Ok(true)
        })?;
        Ok(out)
    }
}

/// The span of one statement, as the OpenTelemetry database conventions name it.
/// Record the rows it read in `db.response.returned_rows`, as an `i64`: the
/// OpenTelemetry layer keeps a `u64` as text.
pub(crate) fn statement_span(sql: &str) -> tracing::Span {
    tracing::info_span!(
        "SELECT",
        otel.kind = "client",
        db.system.name = "sqlite",
        db.query.text = sql,
        db.response.returned_rows = tracing::field::Empty,
    )
}

/// The `SELECT` of each day, joined with `UNION ALL`.
pub(crate) fn union(days: &[Day], part: impl Fn(&str) -> String) -> String {
    days.iter()
        .map(|day| part(&format!("\"{day}\"")))
        .collect::<Vec<_>>()
        .join(" UNION ALL ")
}

/// Cuts `rows` to `limit` and says whether it cut any.
pub(crate) fn cut<T>(rows: &mut Vec<T>, limit: usize) -> bool {
    let truncated = rows.len() > limit;
    rows.truncate(limit);
    truncated
}
