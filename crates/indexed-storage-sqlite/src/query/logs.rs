use std::collections::{BTreeSet, HashMap};
use std::ops::ControlFlow;

use anyhow::ensure;
use otelo_indexed_storage::Severity;
use otelo_indexed_storage::query::{
    LogGroup, LogGroups, LogLine, Logs, MAX_GROUPED_LOG_LINES, PageRequest,
    replace_values_in_message,
};
use otelo_query::{Query, Signal};
use rusqlite::Row;

use super::compile::{QueryContext, TableAliases, compile_query};
use super::{
    LOG_ALIASES, OrderColumn, PageOrder, SortDirection, WhereClause, row_limit_with_one_more,
    span_id_from_blob, split_page, timestamp_from_nanos, trace_id_from_blob, truncate_to_limit,
};
use crate::Reader;
use crate::catalog::AttributeOwner;

const MAX_SAMPLES_PER_GROUP: usize = 3;

const NEWEST_LOGS_FIRST: PageOrder = PageOrder {
    first_column: OrderColumn::new("log.logged_at", SortDirection::Descending),
    later_columns: &[OrderColumn::new("log.rowid", SortDirection::Descending)],
};

pub(super) fn compile_log_query(
    reader: &Reader,
    query: &Query,
) -> anyhow::Result<(WhereClause, Vec<String>)> {
    ensure!(
        query.signal == Signal::Logs,
        "the query is over {}, not logs",
        query.signal
    );
    let mut where_clause = WhereClause::within_reader_range(reader, "log.logged_at");
    let stored_attributes = reader.read_stored_attributes_of_query(AttributeOwner::Log, query)?;
    let unindexed = compile_query(
        query,
        &QueryContext {
            aliases: TableAliases::EncodedRecords(LOG_ALIASES),
            indexed_attributes: reader.indexed_attributes(),
            stored_attributes: &stored_attributes,
        },
        "q",
        &mut where_clause,
    )?;
    Ok((where_clause, unindexed))
}

fn log_line_columns() -> String {
    format!(
        "log.logged_at, resource.service, log.severity_number, log.body, log.trace_id,
         log.span_id, {} AS attributes, resource.attributes AS resource_attributes",
        LOG_ALIASES.record_attributes_json_sql()
    )
}

pub(super) fn explain_logs(reader: &Reader, query: &Query) -> anyhow::Result<Vec<String>> {
    let (where_clause, _) = compile_log_query(reader, query)?;
    reader.explain_scan(
        &format!(
            "{} ORDER BY log.logged_at DESC",
            select_logs(&log_line_columns(), &where_clause)
        ),
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

fn select_logs(columns: &str, where_clause: &WhereClause) -> String {
    format!(
        "SELECT {columns}
         FROM {}
         WHERE {}",
        LOG_ALIASES.encoded_records_sql("logs"),
        where_clause.sql()
    )
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
    let resource: String = row.get(7)?;
    Ok(LogLine {
        logged_at: timestamp_from_nanos(row.get(0)?),
        service: row.get(1)?,
        severity: Severity::from_number(row.get(2)?),
        body: row.get(3)?,
        trace_id: trace_id.map(trace_id_from_blob).transpose()?,
        span_id: span_id.map(span_id_from_blob).transpose()?,
        attributes: serde_json::from_str(&attributes)?,
        resource: serde_json::from_str(&resource)?,
    })
}

pub(super) fn read_logs(
    reader: &Reader,
    query: &Query,
    page: &PageRequest,
) -> anyhow::Result<Logs> {
    let (mut where_clause, unindexed) = compile_log_query(reader, query)?;
    NEWEST_LOGS_FIRST.push_condition_after(&mut where_clause, page.after.as_ref())?;
    where_clause.push_param(":limit", row_limit_with_one_more(page.limit)?);
    let sql = format!(
        "{}
         ORDER BY {}
         LIMIT :limit",
        select_logs(
            &format!(
                "{}, {}",
                log_line_columns(),
                NEWEST_LOGS_FIRST.cursor_columns_sql()
            ),
            &where_clause
        ),
        NEWEST_LOGS_FIRST.order_by_sql()
    );
    let rows = reader.collect_rows(&sql, &where_clause, |row| {
        Ok((NEWEST_LOGS_FIRST.read_cursor(row)?, log_line_from_row(row)?))
    })?;
    let (logs, next) = split_page(rows, page.limit);
    Ok(Logs {
        logs,
        next,
        unindexed,
    })
}

pub(super) fn group_logs(
    reader: &Reader,
    query: &Query,
    limit: usize,
) -> anyhow::Result<LogGroups> {
    let (where_clause, unindexed) = compile_log_query(reader, query)?;
    let sql = format!(
        "{} ORDER BY log.logged_at DESC LIMIT {}",
        select_logs(
            "log.logged_at, resource.service, log.severity_number, log.body",
            &where_clause
        ),
        MAX_GROUPED_LOG_LINES + 1
    );
    let mut groups: HashMap<String, LogGroupTally> = HashMap::new();
    let mut scanned = 0;
    let mut partial = false;
    reader.scan_rows(&sql, &where_clause, |row| {
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
    })?;
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
