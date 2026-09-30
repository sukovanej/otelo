use std::collections::{BTreeSet, HashMap};
use std::ops::ControlFlow;

use anyhow::{Context, ensure};
use otelo_query::{Query, Signal};
use otelo_storage::query::{
    LogGroup, LogGroups, LogLine, Logs, MAX_GROUPED_LOG_LINES, replace_values_in_message,
};
use otelo_storage::{LogSource, Severity};
use rusqlite::Row;

use super::compile::{TableAliases, compile_query};
use super::{
    DaySchema, WhereClause, span_id_from_blob, timestamp_from_nanos, trace_id_from_blob,
    truncate_to_limit,
};
use crate::Reader;

const MAX_SAMPLES_PER_GROUP: usize = 3;

fn compile_log_query(reader: &Reader, query: &Query) -> anyhow::Result<(WhereClause, Vec<String>)> {
    ensure!(
        query.signal == Signal::Logs,
        "the query is over {}, not logs",
        query.signal
    );
    let mut where_clause = WhereClause::within_reader_range(reader, "l.ts");
    let aliases = TableAliases {
        record: "l",
        resource: "r",
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

const LOG_LINE_COLUMNS: &str = "l.ts, r.service, l.severity, l.body, l.trace_id, l.span_id,
    l.attributes, l.source, r.attributes AS resource";

pub(super) fn explain_logs(reader: &Reader, query: &Query) -> anyhow::Result<Vec<String>> {
    let (where_clause, _) = compile_log_query(reader, query)?;
    reader.explain_scan(
        ["", " ORDER BY ts DESC"],
        select_logs(LOG_LINE_COLUMNS, &where_clause),
        &where_clause,
    )
}

pub(super) fn quote_fts_words(text: &str) -> Option<String> {
    // Quoted words keep FTS5 from reading punctuation as query syntax.
    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

fn select_logs(columns: &str, where_clause: &WhereClause) -> impl Fn(DaySchema) -> String {
    move |day_schema| {
        format!(
            "SELECT {columns} FROM {day_schema}.logs l JOIN {day_schema}.resources r ON r.id = l.resource_id
             WHERE {}",
            where_clause.sql_for_day(day_schema)
        )
    }
}

struct LogGroupTally {
    count: u64,
    severity: Severity,
    services: BTreeSet<String>,
    first_at: i64,
    last_at: i64,
    samples: Vec<String>,
}

fn log_line_from_row(row: &Row) -> anyhow::Result<LogLine> {
    let trace_id: Option<Vec<u8>> = row.get(4)?;
    let span_id: Option<Vec<u8>> = row.get(5)?;
    let attributes: String = row.get(6)?;
    let source: String = row.get(7)?;
    let resource: String = row.get(8)?;
    Ok(LogLine {
        logged_at: timestamp_from_nanos(row.get(0)?),
        service: row.get(1)?,
        severity: Severity::from_number(row.get(2)?),
        body: row.get(3)?,
        trace_id: trace_id.map(trace_id_from_blob).transpose()?,
        span_id: span_id.map(span_id_from_blob).transpose()?,
        attributes: serde_json::from_str(&attributes)?,
        resource: serde_json::from_str(&resource)?,
        source: LogSource::from_name(&source)
            .with_context(|| format!("{source:?} is not the source of a log line"))?,
    })
}

pub(super) fn read_logs(reader: &Reader, query: &Query, limit: usize) -> anyhow::Result<Logs> {
    let (where_clause, unindexed) = compile_log_query(reader, query)?;
    let tail = format!(" ORDER BY ts DESC LIMIT {}", limit + 1);
    let mut logs = reader.collect_rows(
        ["", &tail],
        select_logs(LOG_LINE_COLUMNS, &where_clause),
        &where_clause,
        log_line_from_row,
    )?;
    let truncated = truncate_to_limit(&mut logs, limit);
    Ok(Logs {
        logs,
        truncated,
        unindexed,
    })
}

pub(super) fn group_logs(
    reader: &Reader,
    query: &Query,
    limit: usize,
) -> anyhow::Result<LogGroups> {
    let (where_clause, unindexed) = compile_log_query(reader, query)?;
    let columns = "l.ts, r.service, l.severity, l.body";
    let tail = format!(" ORDER BY ts DESC LIMIT {}", MAX_GROUPED_LOG_LINES + 1);
    let mut groups: HashMap<String, LogGroupTally> = HashMap::new();
    let mut scanned = 0;
    let mut partial = false;
    reader.scan_rows(
        ["", &tail],
        select_logs(columns, &where_clause),
        &where_clause,
        |row| {
            if scanned == MAX_GROUPED_LOG_LINES {
                partial = true;
                return Ok(ControlFlow::Break(()));
            }
            scanned += 1;
            let logged_at: i64 = row.get(0)?;
            let service: String = row.get(1)?;
            let severity = Severity::from_number(row.get(2)?);
            let body: String = row.get(3)?;
            let tally = groups
                .entry(replace_values_in_message(&body))
                .or_insert_with(|| LogGroupTally {
                    count: 0,
                    severity,
                    services: BTreeSet::new(),
                    first_at: logged_at,
                    last_at: logged_at,
                    samples: Vec::new(),
                });
            tally.count += 1;
            tally.severity = tally.severity.max(severity);
            tally.services.insert(service);
            // Rows come newest first, so each row moves the first line back.
            tally.first_at = logged_at;
            if tally.samples.len() < MAX_SAMPLES_PER_GROUP && !tally.samples.contains(&body) {
                tally.samples.push(body);
            }
            Ok(ControlFlow::Continue(()))
        },
    )?;
    let mut groups: Vec<LogGroup> = groups
        .into_iter()
        .map(|(template, tally)| LogGroup {
            template,
            count: tally.count,
            severity: tally.severity,
            services: tally.services.into_iter().collect(),
            first_at: timestamp_from_nanos(tally.first_at),
            last_at: timestamp_from_nanos(tally.last_at),
            samples: tally.samples,
        })
        .collect();
    groups
        .sort_by(|a, b| (b.count, b.last_at, &a.template).cmp(&(a.count, a.last_at, &b.template)));
    let truncated = truncate_to_limit(&mut groups, limit);
    Ok(LogGroups {
        groups,
        truncated,
        scanned,
        partial,
        unindexed,
    })
}
