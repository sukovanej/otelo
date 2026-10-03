use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::ops::ControlFlow;

use otelo_indexed_storage::query::{
    CallDetail, CallOperation, Calls, OperationDetail, RequestBucket, Target, TargetKey,
    TargetType, replace_ids_in_path, replace_values_in_query,
};
use otelo_indexed_storage::{SpanKind, SpanStatus};
use otelo_query::quote_string;
use rusqlite::types::Value;

use super::services::{NewestRequest, OperationTally, RequestTally, SpanRowid};
use super::{WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;
use crate::indexes::attribute_json_path;

fn span_attribute(key: &str) -> String {
    format!(
        "json_extract(span.attributes, {})",
        attribute_json_path(key)
    )
}

// An in-process database such as SQLite may put its spans on the internal kind, so a database system marks a call too.
fn call_span_condition() -> String {
    format!(
        "(span.kind IN ({}, {}) OR {} IS NOT NULL OR {} IS NOT NULL)",
        SpanKind::Client.number(),
        SpanKind::Producer.number(),
        span_attribute("db.system.name"),
        span_attribute("db.system")
    )
}

const CALL_ATTRIBUTE_KEYS: [&str; 25] = [
    "db.system.name",
    "db.system",
    "db.namespace",
    "db.name",
    "db.query.summary",
    "db.query.text",
    "db.statement",
    "rpc.system",
    "rpc.service",
    "messaging.system",
    "messaging.destination.name",
    "messaging.destination",
    "http.request.method",
    "http.method",
    "url.full",
    "http.url",
    "url.template",
    "url.path",
    "http.target",
    "server.address",
    "net.peer.name",
    "http.host",
    "server.port",
    "net.peer.port",
    "peer.service",
];

const MAX_NEWEST_CALL_SPANS: usize = 50;

struct CallAttributes([Option<String>; CALL_ATTRIBUTE_KEYS.len()]);

impl CallAttributes {
    fn value_of(&self, key: &str) -> Option<&str> {
        let index = CALL_ATTRIBUTE_KEYS
            .iter()
            .position(|candidate| *candidate == key)?;
        self.0[index].as_deref()
    }

    fn find_first_present<'a>(&'a self, keys: &[&'static str]) -> Option<(&'static str, &'a str)> {
        keys.iter()
            .find_map(|&key| Some((key, self.value_of(key)?)))
    }

    fn is_http(&self) -> bool {
        self.find_first_present(&["url.full", "http.url", "http.request.method", "http.method"])
            .is_some()
    }
}

fn equals_term(key: &str, value: &str) -> String {
    format!("{key} = {}", quote_string(value))
}

fn join_terms(terms: &[&str]) -> String {
    terms
        .iter()
        .filter(|term| !term.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

fn url_host_with_port(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    (!host.is_empty()).then_some(host)
}

fn url_path_without_query(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| {
        rest.find('/').map_or("", |path_start| &rest[path_start..])
    });
    rest.split(['?', '#']).next().unwrap_or_default()
}

fn classify_target(attributes: &CallAttributes) -> (TargetKey, String) {
    let target_key = |target_type, system: Option<&str>, name: Option<&str>| TargetKey {
        target_type,
        system: system.map(str::to_owned),
        name: name.map(str::to_owned),
    };
    for (target_type, system_keys, name_keys) in [
        (
            TargetType::Database,
            &["db.system.name", "db.system"][..],
            &["db.namespace", "db.name"][..],
        ),
        (TargetType::Rpc, &["rpc.system"], &["rpc.service"]),
        (
            TargetType::Messaging,
            &["messaging.system"],
            &["messaging.destination.name", "messaging.destination"],
        ),
    ] {
        if let Some((system_key, system)) = attributes.find_first_present(system_keys) {
            let name = attributes.find_first_present(name_keys);
            let name_term = name.map_or_else(String::new, |(key, value)| equals_term(key, value));
            return (
                target_key(target_type, Some(system), name.map(|(_, value)| value)),
                join_terms(&[&equals_term(system_key, system), &name_term]),
            );
        }
    }

    if attributes.is_http() {
        if let Some((key, host)) =
            attributes.find_first_present(&["server.address", "net.peer.name", "http.host"])
        {
            let port = attributes
                .find_first_present(&["server.port", "net.peer.port"])
                .map(|(_, port)| port)
                .filter(|port| !matches!(*port, "80" | "443") && !host.contains(':'));
            let name = port.map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
            return (
                target_key(TargetType::Http, None, Some(&name)),
                equals_term(key, host),
            );
        }
        let url = attributes.find_first_present(&["url.full", "http.url"]);
        if let Some((key, host)) = url.and_then(|(key, url)| Some((key, url_host_with_port(url)?)))
        {
            let query = format!("{key} ~ {}", quote_string(&format!("://{host}")));
            return (target_key(TargetType::Http, None, Some(host)), query);
        }
        return (target_key(TargetType::Http, None, None), String::new());
    }

    attributes
        .find_first_present(&["peer.service", "server.address"])
        .map_or_else(
            || (target_key(TargetType::Other, None, None), String::new()),
            |(key, peer)| {
                (
                    target_key(TargetType::Other, None, Some(peer)),
                    equals_term(key, peer),
                )
            },
        )
}

fn summarize_call(
    attributes: &CallAttributes,
    target_type: TargetType,
    name: &str,
) -> (String, String) {
    let by_name = || (name.to_owned(), equals_term("name", name));
    match target_type {
        TargetType::Database => {
            if let Some((key, summary)) = attributes.find_first_present(&["db.query.summary"]) {
                return (summary.to_owned(), equals_term(key, summary));
            }
            attributes
                .find_first_present(&["db.query.text", "db.statement"])
                .map_or_else(by_name, |(_, query)| {
                    (replace_values_in_query(query), equals_term("name", name))
                })
        }
        TargetType::Http => {
            let method = attributes
                .find_first_present(&["http.request.method", "http.method"])
                .map_or_else(
                    || name.split(' ').next().unwrap_or_default().to_owned(),
                    |(_, method)| method.to_ascii_uppercase(),
                );
            if let Some((key, template)) = attributes.find_first_present(&["url.template"]) {
                return (
                    join_terms(&[&method, template]),
                    join_terms(&[&equals_term("name", name), &equals_term(key, template)]),
                );
            }
            let path = attributes
                .find_first_present(&["url.path", "http.target", "url.full", "http.url"])
                .map(|(_, url)| replace_ids_in_path(url_path_without_query(url)))
                .filter(|path| !path.is_empty());
            path.map_or_else(by_name, |path| {
                (join_terms(&[&method, &path]), equals_term("name", name))
            })
        }
        TargetType::Rpc | TargetType::Messaging | TargetType::Other => by_name(),
    }
}

struct CallSpan {
    target: TargetKey,
    target_query: String,
    summary: String,
    summary_query: String,
    name: String,
    kind: SpanKind,
    started_at: i64,
    duration_ns: i64,
    failed: bool,
    rowid: SpanRowid,
}

type SpanName = String;

#[derive(Default)]
struct TargetTally {
    query: String,
    calls: RequestTally,
    steps: HashMap<i64, RequestTally>,
    operations: HashMap<(String, SpanKind), OperationTally<SpanName>>,
}

struct CallNameAndQuery {
    name: SpanName,
    query: String,
}

fn fill_request_buckets(
    steps: &HashMap<i64, RequestTally>,
    first_step_at: i64,
    end_at: i64,
    step_ns: i64,
) -> Vec<RequestBucket> {
    let empty = RequestTally::default();
    (first_step_at..end_at)
        .step_by(usize::try_from(step_ns).unwrap_or(usize::MAX))
        .map(|start_at| RequestBucket {
            start_at: timestamp_from_nanos(start_at),
            requests: steps.get(&start_at).unwrap_or(&empty).to_requests(),
        })
        .collect()
}

impl Reader {
    fn scan_call_spans(
        &self,
        service: &str,
        only_kind: Option<SpanKind>,
        mut on_call: impl FnMut(CallSpan),
    ) -> anyhow::Result<()> {
        let mut where_clause = WhereClause::within_reader_range(self, "span.started_at");
        where_clause.push_condition(call_span_condition());
        where_clause.push_condition_with_param(
            "resource.service = :service",
            ":service",
            service.to_owned(),
        );
        if let Some(kind) = only_kind {
            where_clause.push_condition_with_param("span.kind = :kind", ":kind", kind.number());
        }
        let attribute_columns = CALL_ATTRIBUTE_KEYS
            .iter()
            .copied()
            .map(span_attribute)
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT span.name, span.kind, span.started_at, span.duration_ns, span.status_code,
                    span.rowid, {attribute_columns}
             FROM spans span
             JOIN resources resource ON resource.id = span.resource_id
             WHERE {}",
            where_clause.sql()
        );
        self.scan_rows(&sql, &where_clause, |row| {
            let mut attributes = CallAttributes(Default::default());
            for (index, value) in attributes.0.iter_mut().enumerate() {
                *value = match row.get::<_, Value>(6 + index)? {
                    Value::Text(text) if !text.is_empty() => Some(text),
                    Value::Integer(number) => Some(number.to_string()),
                    Value::Real(number) => Some(number.to_string()),
                    _ => None,
                };
            }
            let name: String = row.get(0)?;
            let (target, target_query) = classify_target(&attributes);
            let (summary, summary_query) = summarize_call(&attributes, target.target_type, &name);
            on_call(CallSpan {
                target,
                target_query,
                summary,
                summary_query,
                name,
                kind: SpanKind::from_number(row.get(1)?),
                started_at: row.get(2)?,
                duration_ns: row.get(3)?,
                failed: SpanStatus::from_number(row.get(4)?).is_error(),
                rowid: SpanRowid(row.get(5)?),
            });
            Ok(ControlFlow::Continue(()))
        })
    }
}

fn keep_busiest_operations(
    reader: &Reader,
    targets: &mut HashMap<TargetKey, TargetTally>,
    limit: usize,
) -> anyhow::Result<(HashMap<TargetKey, Vec<CallOperation>>, bool)> {
    let mut operations: Vec<_> = targets
        .iter_mut()
        .flat_map(|(key, target)| {
            target
                .operations
                .drain()
                .map(move |(operation_key, operation)| (key.clone(), operation_key, operation))
        })
        .collect();
    operations.sort_by(|(a_key, a_operation_key, a), (b_key, b_operation_key, b)| {
        b.requests
            .total_ns
            .cmp(&a.requests.total_ns)
            .then_with(|| a_key.cmp(b_key))
            .then_with(|| a_operation_key.cmp(b_operation_key))
    });
    let truncated = truncate_to_limit(&mut operations, limit);
    let mut attributes_by_location =
        reader.read_span_attributes(operations.iter().filter_map(|(_, _, operation)| {
            operation
                .newest_request
                .as_ref()
                .map(|newest_request| newest_request.rowid)
        }))?;
    let mut operations_by_target: HashMap<TargetKey, Vec<CallOperation>> = HashMap::new();
    for (key, (summary, kind), operation) in operations {
        let (name, attributes) = operation
            .newest_request
            .map(|newest_request| {
                (
                    newest_request.detail,
                    attributes_by_location.remove(&newest_request.rowid),
                )
            })
            .unwrap_or_default();
        operations_by_target
            .entry(key)
            .or_default()
            .push(CallOperation {
                summary,
                name,
                kind,
                attributes: attributes.unwrap_or_default(),
                calls: operation.requests.to_requests(),
            });
    }
    Ok((operations_by_target, truncated))
}

pub(super) fn summarize_calls(
    reader: &Reader,
    service: &str,
    step_ns: i64,
    limit: usize,
) -> anyhow::Result<Calls> {
    reader.check_step(step_ns)?;
    let mut calls = RequestTally::default();
    let mut steps: HashMap<i64, RequestTally> = HashMap::new();
    let mut targets: HashMap<TargetKey, TargetTally> = HashMap::new();
    reader.scan_call_spans(service, None, |call| {
        let step_at = call.started_at.div_euclid(step_ns) * step_ns;
        calls.add_request(call.duration_ns, call.failed);
        steps
            .entry(step_at)
            .or_default()
            .add_request(call.duration_ns, call.failed);
        let target = targets.entry(call.target).or_default();
        if target.query.is_empty() {
            target.query = call.target_query;
        }
        target.calls.add_request(call.duration_ns, call.failed);
        target
            .steps
            .entry(step_at)
            .or_default()
            .add_request(call.duration_ns, call.failed);
        target
            .operations
            .entry((call.summary, call.kind))
            .or_default()
            .add_request_at(
                call.started_at,
                call.duration_ns,
                call.failed,
                call.rowid,
                || call.name,
            );
    })?;

    let (mut operations_by_target, truncated) =
        keep_busiest_operations(reader, &mut targets, limit)?;

    let range = reader.range();
    let first_step_at = range.start_at().div_euclid(step_ns) * step_ns;
    let mut targets: Vec<_> = targets
        .into_iter()
        .map(|(key, target)| Target {
            query: target.query,
            calls: target.calls.to_requests(),
            buckets: fill_request_buckets(&target.steps, first_step_at, range.end_at(), step_ns),
            operations: operations_by_target.remove(&key).unwrap_or_default(),
            key,
        })
        .collect();
    targets.sort_by(|a, b| {
        b.calls
            .total_ns
            .cmp(&a.calls.total_ns)
            .then_with(|| a.key.cmp(&b.key))
    });
    Ok(Calls {
        service: service.to_owned(),
        start_at: timestamp_from_nanos(range.start_at()),
        end_at: timestamp_from_nanos(range.end_at()),
        step_ns,
        calls: calls.to_requests(),
        buckets: fill_request_buckets(&steps, first_step_at, range.end_at(), step_ns),
        targets,
        truncated,
    })
}

pub(super) fn summarize_call_operation(
    reader: &Reader,
    service: &str,
    target: &TargetKey,
    summary: &str,
    kind: SpanKind,
    step_ns: i64,
) -> anyhow::Result<CallDetail> {
    reader.check_step(step_ns)?;
    let mut operation: OperationTally<CallNameAndQuery> = OperationTally::default();
    let mut steps: HashMap<i64, RequestTally> = HashMap::new();
    let mut newest_calls: BinaryHeap<Reverse<NewestRequest>> = BinaryHeap::new();
    reader.scan_call_spans(service, Some(kind), |call| {
        if call.target != *target || call.summary != summary {
            return;
        }
        operation.add_request_at(
            call.started_at,
            call.duration_ns,
            call.failed,
            call.rowid,
            || CallNameAndQuery {
                query: join_terms(&[&call.target_query, &call.summary_query]),
                name: call.name,
            },
        );
        steps
            .entry(call.started_at.div_euclid(step_ns) * step_ns)
            .or_default()
            .add_request(call.duration_ns, call.failed);
        newest_calls.push(Reverse(NewestRequest {
            started_at: call.started_at,
            rowid: call.rowid,
            detail: (),
        }));
        if newest_calls.len() > MAX_NEWEST_CALL_SPANS {
            newest_calls.pop();
        }
    })?;
    let (name, query, attributes) = match operation.newest_request {
        Some(NewestRequest { rowid, detail, .. }) => (
            detail.name,
            detail.query,
            reader
                .read_span_attributes(std::iter::once(rowid))?
                .remove(&rowid),
        ),
        None => (String::new(), String::new(), None),
    };
    let rowids: Vec<_> = newest_calls
        .into_iter()
        .map(|Reverse(newest_request)| newest_request.rowid)
        .collect();
    let range = reader.range();
    let first_step_at = range.start_at().div_euclid(step_ns) * step_ns;
    Ok(CallDetail {
        operation: OperationDetail {
            service: service.to_owned(),
            name,
            kind,
            attributes: attributes.unwrap_or_default(),
            start_at: timestamp_from_nanos(range.start_at()),
            end_at: timestamp_from_nanos(range.end_at()),
            step_ns,
            requests: operation.requests.to_requests(),
            buckets: fill_request_buckets(&steps, first_step_at, range.end_at(), step_ns),
        },
        target: target.clone(),
        summary: summary.to_owned(),
        query,
        spans: reader.read_spans_at(&rowids)?,
    })
}
