use std::path::Path;

use otelo_storage::query::{TargetKey, TargetType, path_template, query_template};
use otelo_storage::{
    Attributes, Batch, RangeQueries, Records, Resource, Span, SpanId, SpanKind, SpanStatus,
    TimeRange, TraceId, batch_channel,
};
use otelo_storage_sqlite::{Config, Day, Reader, Writer};
use serde_json::{Value, json};

const SECOND: i64 = 1_000_000_000;
const MS: i64 = 1_000_000;
const MINUTE: i64 = 60 * SECOND;

fn records(service: &str, spans: Vec<Span>) -> Records {
    Records {
        resource: Resource {
            service: service.into(),
            attributes: Attributes::new(),
        },
        logs: Vec::new(),
        spans,
        metrics: Vec::new(),
    }
}

fn child_span(
    id: u8,
    kind: SpanKind,
    name: &str,
    started_at: i64,
    ms: i64,
    attributes: &Value,
) -> Span {
    Span {
        trace_id: TraceId([1; 16]),
        span_id: SpanId([id; 8]),
        parent_span_id: Some(SpanId([1; 8])),
        name: name.into(),
        kind,
        started_at,
        duration_ns: ms * MS,
        status: SpanStatus::Unset,
        attributes: serde_json::from_value(attributes.clone()).unwrap(),
        events: Vec::new(),
    }
}

fn write_batch(dir: &Path, batch: Batch) {
    let (sender, inbox) = batch_channel(1);
    assert!(sender.send(batch));
    let writer = Writer::spawn(Config::new(dir.to_owned()), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn target_key(target_type: TargetType, system: Option<&str>, name: Option<&str>) -> TargetKey {
    TargetKey {
        target_type,
        system: system.map(str::to_owned),
        name: name.map(str::to_owned),
    }
}

fn postgres(query: &str) -> Value {
    json!({"db.system.name": "postgresql", "db.namespace": "app", "db.query.text": query})
}

fn stripe_get(url: &Value) -> Value {
    let mut attributes = json!({
        "http.request.method": "GET",
        "server.address": "api.stripe.com",
        "server.port": 443,
    });
    attributes
        .as_object_mut()
        .unwrap()
        .extend(url.as_object().unwrap().clone());
    attributes
}

fn api_spans(s: i64) -> Vec<Span> {
    vec![
        Span {
            parent_span_id: None,
            ..child_span(1, SpanKind::Server, "GET /users", s, 100, &json!({}))
        },
        // Two queries that differ in a value, under one span name
        // with a third query.
        child_span(
            2,
            SpanKind::Client,
            "SELECT",
            s,
            10,
            &postgres("SELECT * FROM users WHERE id = 1"),
        ),
        Span {
            status: SpanStatus::Error,
            ..child_span(
                3,
                SpanKind::Client,
                "SELECT",
                s + MINUTE,
                20,
                &json!({
                    "db.system.name": "postgresql",
                    "db.namespace": "app",
                    "db.query.text": "SELECT *\n  FROM users WHERE id = 2",
                    "newest": "yes",
                }),
            )
        },
        child_span(
            4,
            SpanKind::Client,
            "SELECT",
            s,
            3,
            &postgres("SELECT * FROM orders WHERE id IN (1, 2, 3) AND note = 'it''s'"),
        ),
        // An in-process database on a span of the internal kind, by
        // the older name of the system, without a query.
        child_span(
            5,
            SpanKind::Internal,
            "SELECT users",
            s,
            5,
            &json!({"db.system": "sqlite"}),
        ),
        child_span(
            6,
            SpanKind::Client,
            "GET",
            s,
            40,
            &stripe_get(&json!({"url.full": "https://api.stripe.com/v1/customers/42?expand=x"})),
        ),
        child_span(
            7,
            SpanKind::Client,
            "GET",
            s,
            2,
            &stripe_get(&json!({"url.template": "/v1/prices/{id}"})),
        ),
        child_span(
            8,
            SpanKind::Client,
            "POST",
            s,
            15,
            &json!({"http.method": "POST", "http.url": "http://localhost:8080/hook?x=1"}),
        ),
        child_span(
            9,
            SpanKind::Producer,
            "orders publish",
            s,
            1,
            &json!({"messaging.system": "kafka", "messaging.destination.name": "orders"}),
        ),
        // Neither is a call.
        child_span(10, SpanKind::Internal, "render", s, 50, &json!({})),
        child_span(11, SpanKind::Server, "POST /orders", s, 50, &json!({})),
    ]
}

struct Fixture {
    dir: tempfile::TempDir,
    today_start_at: i64,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let today_start_at = Day::today().start();
        let s = today_start_at + SECOND;
        let api = records("api", api_spans(s));
        let worker = records(
            "worker",
            vec![child_span(
                12,
                SpanKind::Client,
                "SELECT",
                s,
                10,
                &postgres("SELECT * FROM users WHERE id = 3"),
            )],
        );
        write_batch(dir.path(), vec![api, worker]);
        Self {
            dir,
            today_start_at,
        }
    }

    fn reader(&self) -> Reader {
        let range = TimeRange::new(self.today_start_at, self.today_start_at + 3 * MINUTE).unwrap();
        Reader::open(self.dir.path(), range).unwrap()
    }
}

const USERS: &str = "SELECT * FROM users WHERE id = ?";

#[test]
fn calls_go_by_target_the_most_time_first() {
    let fixture = Fixture::new();
    let calls = fixture.reader().calls("api", MINUTE, 10).unwrap();
    assert_eq!((calls.calls.count, calls.calls.errors), (8, 1));
    assert_eq!(calls.calls.total_ns, 96 * MS);
    let counts: Vec<_> = calls.buckets.iter().map(|b| b.requests.count).collect();
    assert_eq!(counts, [7, 1, 0]);
    assert!(!calls.truncated);

    let targets: Vec<_> = calls
        .targets
        .iter()
        .map(|t| (t.key.clone(), t.query.as_str(), t.calls.count))
        .collect();
    assert_eq!(
        targets,
        [
            (
                target_key(TargetType::Http, None, Some("api.stripe.com")),
                r#"server.address = "api.stripe.com""#,
                2
            ),
            (
                target_key(TargetType::Database, Some("postgresql"), Some("app")),
                r#"db.system.name = "postgresql" db.namespace = "app""#,
                3
            ),
            (
                target_key(TargetType::Http, None, Some("localhost:8080")),
                r#"http.url ~ "://localhost:8080""#,
                1
            ),
            (
                target_key(TargetType::Database, Some("sqlite"), None),
                r#"db.system = "sqlite""#,
                1
            ),
            (
                target_key(TargetType::Messaging, Some("kafka"), Some("orders")),
                r#"messaging.system = "kafka" messaging.destination.name = "orders""#,
                1
            ),
        ]
    );

    let summaries = |at: usize| -> Vec<_> {
        calls.targets[at]
            .operations
            .iter()
            .map(|o| (o.summary.as_str(), o.calls.count))
            .collect()
    };
    assert_eq!(
        summaries(0),
        [("GET /v1/customers/{id}", 1), ("GET /v1/prices/{id}", 1)]
    );
    assert_eq!(
        summaries(1),
        [
            (USERS, 2),
            ("SELECT * FROM orders WHERE id IN (?) AND note = ?", 1)
        ]
    );
    assert_eq!(summaries(2), [("POST /hook", 1)]);
    assert_eq!(summaries(3), [("SELECT users", 1)]);
    assert_eq!(summaries(4), [("orders publish", 1)]);

    let postgres = &calls.targets[1];
    let counts: Vec<_> = postgres.buckets.iter().map(|b| b.requests.count).collect();
    assert_eq!(counts, [2, 1, 0]);
    let users = &postgres.operations[0];
    assert_eq!(
        (users.name.as_str(), users.kind),
        ("SELECT", SpanKind::Client)
    );
    assert_eq!(users.calls.errors, 1);
    assert_eq!(users.attributes["newest"], "yes");
}

#[test]
fn the_limit_keeps_the_operations_with_the_most_time() {
    let fixture = Fixture::new();
    let calls = fixture.reader().calls("api", MINUTE, 2).unwrap();
    assert!(calls.truncated);
    assert_eq!(calls.targets.len(), 5);
    let kept: Vec<_> = calls.targets.iter().map(|t| t.operations.len()).collect();
    assert_eq!(kept, [1, 1, 0, 0, 0]);

    let none = fixture.reader().calls("nobody", MINUTE, 10).unwrap();
    assert_eq!(none.calls.count, 0);
    assert!(none.targets.is_empty());
    assert_eq!(none.buckets.len(), 3);
}

#[test]
fn a_call_is_what_it_does_to_one_target() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let postgres = target_key(TargetType::Database, Some("postgresql"), Some("app"));
    let users = reader
        .call("api", &postgres, USERS, SpanKind::Client, MINUTE)
        .unwrap();
    let detail = &users.detail;
    assert_eq!((detail.requests.count, detail.requests.errors), (2, 1));
    let counts: Vec<_> = detail.buckets.iter().map(|b| b.requests.count).collect();
    assert_eq!(counts, [1, 1, 0]);
    assert_eq!(detail.name, "SELECT");
    assert_eq!(detail.attributes["newest"], "yes");
    assert_eq!(
        users.query,
        r#"db.system.name = "postgresql" db.namespace = "app" name = "SELECT""#
    );
    let ids: Vec<_> = users.spans.iter().map(|s| s.span_id).collect();
    assert_eq!(ids, [SpanId([3; 8]), SpanId([2; 8])]);

    let stripe = target_key(TargetType::Http, None, Some("api.stripe.com"));
    let prices = reader
        .call(
            "api",
            &stripe,
            "GET /v1/prices/{id}",
            SpanKind::Client,
            MINUTE,
        )
        .unwrap();
    assert_eq!(
        prices.query,
        r#"server.address = "api.stripe.com" name = "GET" url.template = "/v1/prices/{id}""#
    );

    let sqlite = target_key(TargetType::Database, Some("sqlite"), None);
    let other = reader
        .call("api", &sqlite, "SELECT users", SpanKind::Internal, MINUTE)
        .unwrap();
    assert_eq!(other.detail.requests.count, 1);
    let none = reader
        .call("api", &sqlite, "SELECT users", SpanKind::Client, MINUTE)
        .unwrap();
    assert_eq!(none.detail.requests.count, 0);
    assert!(none.detail.attributes.is_empty());
    assert!(none.spans.is_empty());
    assert_eq!(none.query, "");
}

#[test]
fn values_and_ids_are_taken_out() {
    for (query, template) in [
        (
            "SELECT a FROM t WHERE id = 7",
            "SELECT a FROM t WHERE id = ?",
        ),
        ("select 1.5, -2, 'x', 'it''s'", "select ?, -?"),
        (
            "INSERT INTO t VALUES (1, 'a'), (2, 'b')",
            "INSERT INTO t VALUES (?)",
        ),
        (
            r#"SELECT * FROM "2026-09-29".spans WHERE a = $1 AND t2.b = ?2"#,
            r#"SELECT * FROM "2026-09-29".spans WHERE a = ? AND t2.b = ?"#,
        ),
        (
            "SELECT x::text FROM t WHERE id IN (:t0, :t1, :t2) AND ts >= :since",
            "SELECT x::text FROM t WHERE id IN (?) AND ts >= ?",
        ),
        ("GET user:42", "GET user:42"),
        ("  SELECT\n\t1  ", "SELECT ?"),
    ] {
        assert_eq!(query_template(query), template, "{query}");
    }
    assert_eq!(
        path_template("/users/42/orders/3fa85f64-5717-4562-b3fc-2c963f66afa6/items/abc"),
        "/users/{id}/orders/{id}/items/abc"
    );
    assert_eq!(path_template("/v1/deadbeef99"), "/v1/{id}");
}
