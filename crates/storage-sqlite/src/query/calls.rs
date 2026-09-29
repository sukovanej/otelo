use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use rusqlite::types::Value;
use siner_query::quote;
use siner_storage::query::{
    CallDetail, CallOperation, Calls, OperationDetail, RequestBucket, Target, TargetKey,
    TargetType, path_template, query_template,
};
use siner_storage::{SpanKind, SpanStatus};

use super::services::{OperationTally, RequestTally, SpanLocation};
use super::{WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;

// An in-process database such as SQLite may put its spans on the internal
// kind, so a database system marks a call too.
const CALL_SPAN_CLAUSE: &str = "(s.kind IN (3, 4)
    OR json_extract(s.attributes, '$.\"db.system.name\"') IS NOT NULL
    OR json_extract(s.attributes, '$.\"db.system\"') IS NOT NULL)";

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
        let at = CALL_ATTRIBUTE_KEYS.iter().position(|k| *k == key)?;
        self.0[at].as_deref()
    }

    fn first_present<'a>(&'a self, keys: &[&'static str]) -> Option<(&'static str, &'a str)> {
        keys.iter()
            .find_map(|&key| Some((key, self.value_of(key)?)))
    }

    fn is_http(&self) -> bool {
        self.first_present(&["url.full", "http.url", "http.request.method", "http.method"])
            .is_some()
    }
}

fn equals_term(key: &str, value: &str) -> String {
    format!("{key} = {}", quote(value))
}

fn join_terms(terms: &[&str]) -> String {
    terms
        .iter()
        .filter(|t| !t.is_empty())
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
    let rest = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest.find('/').map_or("", |at| &rest[at..]));
    rest.split(['?', '#']).next().unwrap_or_default()
}

fn classify_target(attributes: &CallAttributes) -> (TargetKey, String) {
    let target = |target_type, system: Option<&str>, name: Option<&str>| TargetKey {
        target_type,
        system: system.map(str::to_owned),
        name: name.map(str::to_owned),
    };
    for (target_type, systems, names) in [
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
        if let Some((system_key, system)) = attributes.first_present(systems) {
            let name = attributes.first_present(names);
            let name_term = name.map_or_else(String::new, |(key, value)| equals_term(key, value));
            return (
                target(target_type, Some(system), name.map(|(_, value)| value)),
                join_terms(&[&equals_term(system_key, system), &name_term]),
            );
        }
    }

    if attributes.is_http() {
        if let Some((key, host)) =
            attributes.first_present(&["server.address", "net.peer.name", "http.host"])
        {
            let port = attributes
                .first_present(&["server.port", "net.peer.port"])
                .map(|(_, port)| port)
                .filter(|port| !matches!(*port, "80" | "443") && !host.contains(':'));
            let name = port.map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
            return (
                target(TargetType::Http, None, Some(&name)),
                equals_term(key, host),
            );
        }
        let url = attributes.first_present(&["url.full", "http.url"]);
        if let Some((key, host)) = url.and_then(|(key, url)| Some((key, url_host_with_port(url)?)))
        {
            let query = format!("{key} ~ {}", quote(&format!("://{host}")));
            return (target(TargetType::Http, None, Some(host)), query);
        }
        return (target(TargetType::Http, None, None), String::new());
    }

    attributes
        .first_present(&["peer.service", "server.address"])
        .map_or_else(
            || (target(TargetType::Other, None, None), String::new()),
            |(key, peer)| {
                (
                    target(TargetType::Other, None, Some(peer)),
                    equals_term(key, peer),
                )
            },
        )
}

fn summarize_call(attributes: &CallAttributes, target: TargetType, name: &str) -> (String, String) {
    let by_name = || (name.to_owned(), equals_term("name", name));
    match target {
        TargetType::Database => {
            if let Some((key, summary)) = attributes.first_present(&["db.query.summary"]) {
                return (summary.to_owned(), equals_term(key, summary));
            }
            attributes
                .first_present(&["db.query.text", "db.statement"])
                .map_or_else(by_name, |(_, query)| {
                    (query_template(query), equals_term("name", name))
                })
        }
        TargetType::Http => {
            let method = attributes
                .first_present(&["http.request.method", "http.method"])
                .map_or_else(
                    || name.split(' ').next().unwrap_or_default().to_owned(),
                    |(_, method)| method.to_ascii_uppercase(),
                );
            if let Some((key, template)) = attributes.first_present(&["url.template"]) {
                return (
                    join_terms(&[&method, template]),
                    join_terms(&[&equals_term("name", name), &equals_term(key, template)]),
                );
            }
            let path = attributes
                .first_present(&["url.path", "http.target", "url.full", "http.url"])
                .map(|(_, url)| path_template(url_path_without_query(url)))
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
    location: SpanLocation,
}

#[derive(Default)]
struct TargetTally {
    query: String,
    calls: RequestTally,
    steps: HashMap<i64, RequestTally>,
    operations: HashMap<(String, SpanKind), NamedOperationTally>,
}

#[derive(Default)]
struct NamedOperationTally {
    tally: OperationTally,
    newest_name: String,
}

fn request_buckets(
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
        let mut where_ = WhereClause::within_reader_range(self, "s.start_ts");
        where_.push_clause(CALL_SPAN_CLAUSE.into());
        where_.push_clause_with_param("r.service = :service", ":service", service.to_owned());
        if let Some(kind) = only_kind {
            where_.push_clause_with_param("s.kind = :kind", ":kind", kind.number());
        }
        let columns = CALL_ATTRIBUTE_KEYS
            .iter()
            .map(|key| format!("json_extract(s.attributes, '$.\"{key}\"')"))
            .collect::<Vec<_>>()
            .join(", ");
        self.scan_rows(
            ["", ""],
            |day| {
                format!(
                    "SELECT s.name, s.kind, s.start_ts, s.duration_ns, s.status, '{}', s.rowid,
                            {columns}
                     FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    day.trim_matches('"'),
                    where_.sql_for_day(day)
                )
            },
            &where_,
            |row| {
                let mut attributes = CallAttributes(Default::default());
                for (i, value) in attributes.0.iter_mut().enumerate() {
                    *value = match row.get::<_, Value>(7 + i)? {
                        Value::Text(text) if !text.is_empty() => Some(text),
                        Value::Integer(n) => Some(n.to_string()),
                        Value::Real(n) => Some(n.to_string()),
                        _ => None,
                    };
                }
                let name: String = row.get(0)?;
                let (target, target_query) = classify_target(&attributes);
                let (summary, summary_query) =
                    summarize_call(&attributes, target.target_type, &name);
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
                    location: (row.get(5)?, row.get(6)?),
                });
                Ok(true)
            },
        )
    }
}

fn keep_busiest_operations(
    reader: &Reader,
    targets: &mut HashMap<TargetKey, TargetTally>,
    limit: usize,
) -> anyhow::Result<(HashMap<TargetKey, Vec<CallOperation>>, bool)> {
    // The limit keeps the operations with the most time across all targets.
    let mut operations: Vec<_> = targets
        .iter_mut()
        .flat_map(|(key, target)| {
            target
                .operations
                .drain()
                .map(move |(operation_key, operation)| (key.clone(), operation_key, operation))
        })
        .collect();
    operations.sort_by(|(a_key, a_op, a), (b_key, b_op, b)| {
        b.tally
            .requests
            .total_ns
            .cmp(&a.tally.requests.total_ns)
            .then_with(|| a_key.cmp(b_key))
            .then_with(|| a_op.cmp(b_op))
    });
    let truncated = truncate_to_limit(&mut operations, limit);
    let mut attributes = reader.read_span_attributes(
        operations
            .iter()
            .filter_map(|(_, _, operation)| Some(&operation.tally.newest_request.as_ref()?.1)),
    )?;
    let mut operations_by_target: HashMap<TargetKey, Vec<CallOperation>> = HashMap::new();
    for (key, (summary, kind), operation) in operations {
        operations_by_target
            .entry(key)
            .or_default()
            .push(CallOperation {
                summary,
                name: operation.newest_name,
                kind,
                attributes: operation
                    .tally
                    .newest_request
                    .and_then(|(_, location)| attributes.remove(&location))
                    .unwrap_or_default(),
                calls: operation.tally.requests.to_requests(),
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
        let operation = target
            .operations
            .entry((call.summary, call.kind))
            .or_default();
        operation.tally.add_request_at(
            call.started_at,
            call.duration_ns,
            call.failed,
            &call.location,
        );
        if operation.tally.is_newest_request_at(call.started_at) {
            operation.newest_name = call.name;
        }
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
            buckets: request_buckets(&target.steps, first_step_at, range.end_at(), step_ns),
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
        buckets: request_buckets(&steps, first_step_at, range.end_at(), step_ns),
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
    let mut operation = OperationTally::default();
    let mut steps: HashMap<i64, RequestTally> = HashMap::new();
    let mut newest_calls: BinaryHeap<Reverse<(i64, SpanLocation)>> = BinaryHeap::new();
    let (mut name, mut query) = (String::new(), String::new());
    reader.scan_call_spans(service, Some(kind), |call| {
        if call.target != *target || call.summary != summary {
            return;
        }
        operation.add_request_at(
            call.started_at,
            call.duration_ns,
            call.failed,
            &call.location,
        );
        if operation.is_newest_request_at(call.started_at) {
            query = join_terms(&[&call.target_query, &call.summary_query]);
            name = call.name;
        }
        steps
            .entry(call.started_at.div_euclid(step_ns) * step_ns)
            .or_default()
            .add_request(call.duration_ns, call.failed);
        newest_calls.push(Reverse((call.started_at, call.location)));
        if newest_calls.len() > MAX_NEWEST_CALL_SPANS {
            newest_calls.pop();
        }
    })?;
    let attributes = match operation.newest_request {
        Some((_, location)) => reader
            .read_span_attributes(std::iter::once(&location))?
            .remove(&location),
        None => None,
    };
    let locations: Vec<_> = newest_calls
        .into_iter()
        .map(|Reverse((_, location))| location)
        .collect();
    let range = reader.range();
    let first_step_at = range.start_at().div_euclid(step_ns) * step_ns;
    Ok(CallDetail {
        detail: OperationDetail {
            service: service.to_owned(),
            name,
            kind,
            attributes: attributes.unwrap_or_default(),
            start_at: timestamp_from_nanos(range.start_at()),
            end_at: timestamp_from_nanos(range.end_at()),
            step_ns,
            requests: operation.requests.to_requests(),
            buckets: request_buckets(&steps, first_step_at, range.end_at(), step_ns),
        },
        target: target.clone(),
        summary: summary.to_owned(),
        query,
        spans: reader.read_spans_at(&locations)?,
    })
}
