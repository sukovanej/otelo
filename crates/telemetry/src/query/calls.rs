//! The calls a service makes: to a database, over HTTP, over RPC, or to a
//! message broker, by what they call and by what they do there.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use jiff::Timestamp;
use rusqlite::types::Value;
use serde::{Deserialize, Serialize};
use siner_query::quote;
use utoipa::ToSchema;

use super::services::{OperationTally, SpanRow, Tally};
use super::{Filter, OperationDetail, RequestBucket, Requests, STATUS_ERROR, TraceSpan, cut, time};
use crate::{Attributes, Reader};

/// The spans a service makes calls with: of the client or the producer kind,
/// or of a database system, which an in-process database such as SQLite may
/// put on a span of the internal kind.
const CALL: &str = "(s.kind IN (3, 4)
    OR json_extract(s.attributes, '$.\"db.system.name\"') IS NOT NULL
    OR json_extract(s.attributes, '$.\"db.system\"') IS NOT NULL)";

/// The attributes that tell what a call goes to and what it does there,
/// from the OpenTelemetry conventions, the older names too.
const KEYS: [&str; 25] = [
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

/// How many of its newest spans the detail of a call lists.
const NEWEST_SPANS: usize = 50;

/// The calls a service made in a range, by what they went to.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Calls {
    pub service: String,
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub since: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub until: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    /// Every call of the service.
    pub calls: Requests,
    /// The calls of every step, oldest first.
    pub buckets: Vec<RequestBucket>,
    /// What the service called, the most time first.
    pub targets: Vec<Target>,
    /// The targets have more operations than the limit let through, which
    /// kept the ones with the most time.
    pub truncated: bool,
}

/// What kind of thing a call goes to.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum TargetType {
    Database,
    Http,
    Rpc,
    Messaging,
    Other,
}

/// What a call goes to: a database, a host, an RPC service, or a message
/// destination.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct TargetKey {
    #[serde(rename = "type")]
    pub target_type: TargetType,
    /// Such as `postgresql`, `grpc`, or `kafka`. Null for HTTP, and for
    /// a target of no known type.
    #[schema(required = true)]
    pub system: Option<String>,
    /// The database, the host with a port that is not 80 or 443, the RPC
    /// service, the destination, or the `peer.service` of other calls.
    /// Null when the call does not say.
    #[schema(required = true)]
    pub name: Option<String>,
}

/// The calls to one target.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Target {
    #[serde(flatten)]
    pub key: TargetKey,
    /// The terms of a span query that keep the calls to the target, such as
    /// `db.system.name = "postgresql"`, from the attributes of one of them.
    /// Empty when no attribute tells the target.
    pub query: String,
    pub calls: Requests,
    /// The calls of every step, oldest first.
    pub buckets: Vec<RequestBucket>,
    /// The calls by summary and kind, the most time first.
    pub operations: Vec<CallOperation>,
}

/// The calls to a target that do the same thing.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CallOperation {
    /// What the calls do: `db.query.summary`, or the query with its values
    /// as `?`, such as `SELECT * FROM users WHERE id = ?`, for a database;
    /// the method and `url.template`, or the path with its ids as `{id}`,
    /// such as `GET /users/{id}`, for HTTP; and the span name for the rest.
    pub summary: String,
    /// The span name of its newest call.
    pub name: String,
    /// The OpenTelemetry span kind.
    pub kind: i32,
    /// The attributes of its newest call.
    pub attributes: Attributes,
    pub calls: Requests,
}

/// The calls of a service to one target that do one thing, over a range.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct CallDetail {
    /// The calls, as the requests of an operation named after the span name
    /// of the newest one.
    #[serde(flatten)]
    pub detail: OperationDetail,
    pub target: TargetKey,
    /// What the calls do, as [`CallOperation::summary`] says.
    pub summary: String,
    /// The terms of a span query that keep these calls, or more of them when
    /// the summary comes from a query or a path with its values taken out.
    /// Empty without calls in the range.
    pub query: String,
    /// The newest calls, newest first.
    pub spans: Vec<TraceSpan>,
}

/// The attributes of a call that [`KEYS`] name.
struct Keys([Option<String>; KEYS.len()]);

impl Keys {
    fn get(&self, key: &str) -> Option<&str> {
        let at = KEYS.iter().position(|k| *k == key)?;
        self.0[at].as_deref()
    }

    /// The first of `keys` the call has, and its value.
    fn first<'a>(&'a self, keys: &[&'static str]) -> Option<(&'static str, &'a str)> {
        keys.iter().find_map(|&key| Some((key, self.get(key)?)))
    }
}

/// A term of a span query that keeps `key` equal to `value`.
fn equals(key: &str, value: &str) -> String {
    format!("{key} = {}", quote(value))
}

/// `terms` joined into a query, the empty ones left out.
fn join(terms: &[&str]) -> String {
    terms
        .iter()
        .filter(|t| !t.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The host of a URL with its port, such as `api.stripe.com` or
/// `localhost:8080`.
fn url_host(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    (!host.is_empty()).then_some(host)
}

/// The path of a URL, or of a target such as `/users/7?x=1`, without its
/// query.
fn url_path(url: &str) -> &str {
    let rest = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest.find('/').map_or("", |at| &rest[at..]));
    rest.split(['?', '#']).next().unwrap_or_default()
}

/// Whether a segment of a path is an id: a number, a UUID, or a run of hex
/// digits of 8 or more with a digit in it.
fn is_id(segment: &str) -> bool {
    let hex = segment.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    !segment.is_empty()
        && (segment.chars().all(|c| c.is_ascii_digit())
            || (hex && segment.len() >= 8 && segment.chars().any(|c| c.is_ascii_digit())))
}

/// A path with each segment that is an id as `{id}`, so the requests to
/// `/users/7` and `/users/8` are one.
#[must_use]
pub fn path_template(path: &str) -> String {
    path.split('/')
        .map(|segment| if is_id(segment) { "{id}" } else { segment })
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether `c` and then `next` start a parameter: `$1`, `?1`, or `:id`.
fn is_parameter(c: char, next: Option<char>) -> bool {
    next.is_some_and(|d| match c {
        ':' => d.is_ascii_alphabetic() || d == '_',
        _ => d.is_ascii_digit(),
    })
}

/// A query with its values taken out, so the calls that differ only in
/// them are one.
///
/// A string, a number, or a parameter such as `$1`, `?1`, or `:id` is `?`,
/// a list of them one `?`, and whitespace one space. Quoted names stay.
#[must_use]
pub fn query_template(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    let mut chars = query.chars().peekable();
    // Whether the last character is part of a name, where a digit is too.
    let mut in_name = false;
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                while let Some(d) = chars.next() {
                    if d == '\'' && chars.next_if_eq(&'\'').is_none() {
                        break;
                    }
                }
                out.push('?');
                in_name = false;
            }
            '"' | '`' => {
                out.push(c);
                for d in chars.by_ref() {
                    out.push(d);
                    if d == c {
                        break;
                    }
                }
                in_name = true;
            }
            '$' | '?' | ':' if !in_name && is_parameter(c, chars.peek().copied()) => {
                while chars
                    .next_if(|d| d.is_ascii_alphanumeric() || *d == '_')
                    .is_some()
                {}
                out.push('?');
            }
            c if c.is_ascii_digit() && !in_name => {
                while chars
                    .next_if(|d| d.is_ascii_alphanumeric() || *d == '.')
                    .is_some()
                {}
                out.push('?');
            }
            c if c.is_whitespace() => {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                in_name = false;
            }
            c => {
                out.push(c);
                in_name = c.is_alphanumeric() || matches!(c, '_' | '$' | '?' | ':' | '@');
            }
        }
    }
    let mut out = out.trim_end().to_owned();
    for (list, one) in [
        ("?, ?", "?"),
        ("?,?", "?"),
        ("(?), (?)", "(?)"),
        ("(?),(?)", "(?)"),
    ] {
        while out.contains(list) {
            out = out.replace(list, one);
        }
    }
    out
}

/// What a call with these attributes goes to, and the span query terms
/// that keep the calls to it.
fn classify(keys: &Keys) -> (TargetKey, String) {
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
        if let Some((system_key, system)) = keys.first(systems) {
            let name = keys.first(names);
            let name_term = name.map_or_else(String::new, |(key, value)| equals(key, value));
            return (
                target(target_type, Some(system), name.map(|n| n.1)),
                join(&[&equals(system_key, system), &name_term]),
            );
        }
    }

    if is_http(keys) {
        if let Some((key, host)) = keys.first(&["server.address", "net.peer.name", "http.host"]) {
            let port = keys
                .first(&["server.port", "net.peer.port"])
                .map(|(_, port)| port)
                .filter(|port| !matches!(*port, "80" | "443") && !host.contains(':'));
            let name = port.map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
            return (
                target(TargetType::Http, None, Some(&name)),
                equals(key, host),
            );
        }
        let url = keys.first(&["url.full", "http.url"]);
        if let Some((key, host)) = url.and_then(|(key, url)| Some((key, url_host(url)?))) {
            let query = format!("{key} ~ {}", quote(&format!("://{host}")));
            return (target(TargetType::Http, None, Some(host)), query);
        }
        return (target(TargetType::Http, None, None), String::new());
    }

    keys.first(&["peer.service", "server.address"]).map_or_else(
        || (target(TargetType::Other, None, None), String::new()),
        |(key, peer)| {
            (
                target(TargetType::Other, None, Some(peer)),
                equals(key, peer),
            )
        },
    )
}

fn is_http(keys: &Keys) -> bool {
    keys.first(&["url.full", "http.url", "http.request.method", "http.method"])
        .is_some()
}

/// What a call does, as [`CallOperation::summary`] says, and the span query
/// terms that keep the calls that do it: exact for `db.query.summary` and
/// `url.template`, and the span name otherwise.
fn summarize(keys: &Keys, target: TargetType, name: &str) -> (String, String) {
    let by_name = || (name.to_owned(), equals("name", name));
    match target {
        TargetType::Database => {
            if let Some((key, summary)) = keys.first(&["db.query.summary"]) {
                return (summary.to_owned(), equals(key, summary));
            }
            keys.first(&["db.query.text", "db.statement"])
                .map_or_else(by_name, |(_, query)| {
                    (query_template(query), equals("name", name))
                })
        }
        TargetType::Http => {
            let method = keys
                .first(&["http.request.method", "http.method"])
                .map_or_else(
                    || name.split(' ').next().unwrap_or_default().to_owned(),
                    |(_, method)| method.to_ascii_uppercase(),
                );
            if let Some((key, template)) = keys.first(&["url.template"]) {
                return (
                    join(&[&method, template]),
                    join(&[&equals("name", name), &equals(key, template)]),
                );
            }
            let path = keys
                .first(&["url.path", "http.target", "url.full", "http.url"])
                .map(|(_, url)| path_template(url_path(url)))
                .filter(|path| !path.is_empty());
            path.map_or_else(by_name, |path| {
                (join(&[&method, &path]), equals("name", name))
            })
        }
        TargetType::Rpc | TargetType::Messaging | TargetType::Other => by_name(),
    }
}

/// A call a service made.
struct Call {
    target: TargetKey,
    /// The terms of a span query that keep the calls to the target.
    target_query: String,
    summary: String,
    /// The terms of a span query that keep the calls that do the same.
    summary_query: String,
    name: String,
    kind: i32,
    start: i64,
    duration_ns: i64,
    error: bool,
    row: SpanRow,
}

/// The calls to a target, over the range, by step, and by what they do.
#[derive(Default)]
struct TargetTally {
    query: String,
    calls: Tally,
    steps: HashMap<i64, Tally>,
    /// By summary and kind, with the span name of the newest call.
    operations: HashMap<(String, i32), (OperationTally, String)>,
}

/// A bucket for every step from `first` up to `until`.
fn buckets(steps: &HashMap<i64, Tally>, first: i64, until: i64, step: i64) -> Vec<RequestBucket> {
    let empty = Tally::default();
    (first..until)
        .step_by(usize::try_from(step).unwrap_or(usize::MAX))
        .map(|start| RequestBucket {
            time: time(start),
            requests: steps.get(&start).unwrap_or(&empty).finish(),
        })
        .collect()
}

/// Whether `start` is the newest start of `operation`, which has just
/// counted it.
fn is_newest(operation: &OperationTally, start: i64) -> bool {
    operation
        .newest
        .as_ref()
        .is_some_and(|(newest, _)| *newest == start)
}

impl Reader {
    /// The calls `service` made in the range, over it and in buckets of
    /// `step_ns`, by target and by what they do, `limit` operations at most.
    ///
    /// # Errors
    ///
    /// When the step makes more than [`super::MAX_BUCKETS`] buckets, or when
    /// the query fails, such as when it runs past the time limit.
    pub fn calls(&self, service: &str, step_ns: i64, limit: usize) -> anyhow::Result<Calls> {
        self.check_step(step_ns)?;
        let mut calls = Tally::default();
        let mut steps: HashMap<i64, Tally> = HashMap::new();
        let mut targets: HashMap<TargetKey, TargetTally> = HashMap::new();
        self.scan_calls(service, None, |call| {
            let start = call.start.div_euclid(step_ns) * step_ns;
            calls.add(call.duration_ns, call.error);
            steps
                .entry(start)
                .or_default()
                .add(call.duration_ns, call.error);
            let target = targets.entry(call.target).or_default();
            if target.query.is_empty() {
                target.query = call.target_query;
            }
            target.calls.add(call.duration_ns, call.error);
            target
                .steps
                .entry(start)
                .or_default()
                .add(call.duration_ns, call.error);
            let (operation, name) = target
                .operations
                .entry((call.summary, call.kind))
                .or_default();
            operation.add(call.start, call.duration_ns, call.error, &call.row);
            if is_newest(operation, call.start) {
                *name = call.name;
            }
        })?;

        // The operations with the most time of every target, `limit` in all.
        let mut operations: Vec<_> = targets
            .iter_mut()
            .flat_map(|(key, target)| {
                target
                    .operations
                    .drain()
                    .map(move |(operation, tally)| (key.clone(), operation, tally))
            })
            .collect();
        operations.sort_by(|(a_key, a_op, (a, _)), (b_key, b_op, (b, _))| {
            b.requests
                .total_ns
                .cmp(&a.requests.total_ns)
                .then_with(|| a_key.cmp(b_key))
                .then_with(|| a_op.cmp(b_op))
        });
        let truncated = cut(&mut operations, limit);
        let mut attributes = self.span_attributes(
            operations
                .iter()
                .filter_map(|(_, _, (o, _))| Some(&o.newest.as_ref()?.1)),
        )?;
        let mut by_target: HashMap<TargetKey, Vec<CallOperation>> = HashMap::new();
        for (key, (summary, kind), (operation, name)) in operations {
            by_target.entry(key).or_default().push(CallOperation {
                summary,
                name,
                kind,
                attributes: operation
                    .newest
                    .and_then(|(_, row)| attributes.remove(&row))
                    .unwrap_or_default(),
                calls: operation.requests.finish(),
            });
        }

        let first = self.since().div_euclid(step_ns) * step_ns;
        let mut targets: Vec<_> = targets
            .into_iter()
            .map(|(key, target)| Target {
                query: target.query,
                calls: target.calls.finish(),
                buckets: buckets(&target.steps, first, self.until(), step_ns),
                operations: by_target.remove(&key).unwrap_or_default(),
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
            since: time(self.since()),
            until: time(self.until()),
            step_ns,
            calls: calls.finish(),
            buckets: buckets(&steps, first, self.until(), step_ns),
            targets,
            truncated,
        })
    }

    /// The calls of `service` to `target` that do `summary`, of the kind
    /// `kind`, in the range, over it and in buckets of `step_ns`, with the
    /// newest of them.
    ///
    /// # Errors
    ///
    /// When the step makes more than [`super::MAX_BUCKETS`] buckets, or when
    /// the query fails, such as when it runs past the time limit.
    pub fn call(
        &self,
        service: &str,
        target: &TargetKey,
        summary: &str,
        kind: i32,
        step_ns: i64,
    ) -> anyhow::Result<CallDetail> {
        self.check_step(step_ns)?;
        let mut operation = OperationTally::default();
        let mut steps: HashMap<i64, Tally> = HashMap::new();
        let mut newest: BinaryHeap<Reverse<(i64, SpanRow)>> = BinaryHeap::new();
        let (mut name, mut query) = (String::new(), String::new());
        self.scan_calls(service, Some(kind), |call| {
            if call.target != *target || call.summary != summary {
                return;
            }
            operation.add(call.start, call.duration_ns, call.error, &call.row);
            if is_newest(&operation, call.start) {
                query = join(&[&call.target_query, &call.summary_query]);
                name = call.name;
            }
            steps
                .entry(call.start.div_euclid(step_ns) * step_ns)
                .or_default()
                .add(call.duration_ns, call.error);
            newest.push(Reverse((call.start, call.row)));
            if newest.len() > NEWEST_SPANS {
                newest.pop();
            }
        })?;
        let attributes = match operation.newest {
            Some((_, row)) => self.span_attributes(std::iter::once(&row))?.remove(&row),
            None => None,
        };
        let rows: Vec<_> = newest.into_iter().map(|Reverse((_, row))| row).collect();
        let first = self.since().div_euclid(step_ns) * step_ns;
        Ok(CallDetail {
            detail: OperationDetail {
                service: service.to_owned(),
                name,
                kind,
                attributes: attributes.unwrap_or_default(),
                since: time(self.since()),
                until: time(self.until()),
                step_ns,
                requests: operation.requests.finish(),
                buckets: buckets(&steps, first, self.until(), step_ns),
            },
            target: target.clone(),
            summary: summary.to_owned(),
            query,
            spans: self.spans_at(&rows)?,
        })
    }

    /// Hands every call of `service` in the range to `each`, or only the
    /// ones of the kind `only`.
    fn scan_calls(
        &self,
        service: &str,
        only: Option<i32>,
        mut each: impl FnMut(Call),
    ) -> anyhow::Result<()> {
        let mut where_ = Filter::range(self, "s.start_ts");
        where_.push_clause(CALL.into());
        where_.push("r.service = :service", ":service", service.to_owned());
        if let Some(kind) = only {
            where_.push("s.kind = :kind", ":kind", kind);
        }
        let columns = KEYS
            .iter()
            .map(|key| format!("json_extract(s.attributes, '$.\"{key}\"')"))
            .collect::<Vec<_>>()
            .join(", ");
        self.scan(
            ["", ""],
            |day| {
                format!(
                    "SELECT s.name, s.kind, s.start_ts, s.duration_ns, s.status, '{}', s.rowid,
                            {columns}
                     FROM {day}.spans s JOIN {day}.resources r ON r.id = s.resource_id
                     WHERE {}",
                    day.trim_matches('"'),
                    where_.sql(day)
                )
            },
            &where_,
            |row| {
                let mut keys = Keys(Default::default());
                for (i, value) in keys.0.iter_mut().enumerate() {
                    *value = match row.get::<_, Value>(7 + i)? {
                        Value::Text(text) if !text.is_empty() => Some(text),
                        Value::Integer(n) => Some(n.to_string()),
                        Value::Real(n) => Some(n.to_string()),
                        _ => None,
                    };
                }
                let name: String = row.get(0)?;
                let (target, target_query) = classify(&keys);
                let (summary, summary_query) = summarize(&keys, target.target_type, &name);
                each(Call {
                    target,
                    target_query,
                    summary,
                    summary_query,
                    name,
                    kind: row.get(1)?,
                    start: row.get(2)?,
                    duration_ns: row.get(3)?,
                    error: row.get::<_, i32>(4)? == STATUS_ERROR,
                    row: (row.get(5)?, row.get(6)?),
                });
                Ok(true)
            },
        )
    }
}
