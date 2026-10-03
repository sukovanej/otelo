use std::path::Path;

use otelo_indexed_storage::query::{TargetKey, TargetType};
use otelo_indexed_storage::{
    Attributes, Batch, RangeQueries, Records, Resource, Span, SpanId, SpanKind, SpanStatus,
    TimeRange, TraceId, open_batch_channel,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader, Writer};
use serde_json::{Value, json};

const SECOND: i64 = 1_000_000_000;
const MILLISECOND: i64 = 1_000_000;
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
    span_id_byte: u8,
    kind: SpanKind,
    name: &str,
    started_at: i64,
    duration_ms: i64,
    attributes: &Value,
) -> Span {
    Span {
        trace_id: TraceId([1; 16]),
        span_id: SpanId([span_id_byte; 8]),
        parent_span_id: Some(SpanId([1; 8])),
        name: name.into(),
        kind,
        started_at,
        duration_ns: duration_ms * MILLISECOND,
        status: SpanStatus::Unset,
        attributes: serde_json::from_value(attributes.clone()).unwrap(),
        events: Vec::new(),
    }
}

fn write_batch(directory: &Path, batch: Batch) {
    let (sender, inbox) = open_batch_channel(1);
    assert!(sender.send_batch(batch));
    let writer = Writer::spawn(Config::new(directory.to_owned()), inbox).unwrap();
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

fn postgres_attributes(query: &str) -> Value {
    json!({"db.system.name": "postgresql", "db.namespace": "app", "db.query.text": query})
}

fn stripe_get_attributes(url: &Value) -> Value {
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

fn api_spans(started_at: i64) -> Vec<Span> {
    vec![
        Span {
            parent_span_id: None,
            ..child_span(
                1,
                SpanKind::Server,
                "GET /users",
                started_at,
                100,
                &json!({}),
            )
        },
        // Two queries that differ in a value, under one span name with a third query.
        child_span(
            2,
            SpanKind::Client,
            "SELECT",
            started_at,
            10,
            &postgres_attributes("SELECT * FROM users WHERE id = 1"),
        ),
        Span {
            status: SpanStatus::Error,
            ..child_span(
                3,
                SpanKind::Client,
                "SELECT",
                started_at + MINUTE,
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
            started_at,
            3,
            &postgres_attributes("SELECT * FROM orders WHERE id IN (1, 2, 3) AND note = 'it''s'"),
        ),
        // An in-process database on a span of the internal kind, by the older name of the system, without a query.
        child_span(
            5,
            SpanKind::Internal,
            "SELECT users",
            started_at,
            5,
            &json!({"db.system": "sqlite"}),
        ),
        child_span(
            6,
            SpanKind::Client,
            "GET",
            started_at,
            40,
            &stripe_get_attributes(
                &json!({"url.full": "https://api.stripe.com/v1/customers/42?expand=x"}),
            ),
        ),
        child_span(
            7,
            SpanKind::Client,
            "GET",
            started_at,
            2,
            &stripe_get_attributes(&json!({"url.template": "/v1/prices/{id}"})),
        ),
        child_span(
            8,
            SpanKind::Client,
            "POST",
            started_at,
            15,
            &json!({"http.method": "POST", "http.url": "http://localhost:8080/hook?x=1"}),
        ),
        child_span(
            9,
            SpanKind::Producer,
            "orders publish",
            started_at,
            1,
            &json!({"messaging.system": "kafka", "messaging.destination.name": "orders"}),
        ),
        // Neither is a call.
        child_span(10, SpanKind::Internal, "render", started_at, 50, &json!({})),
        child_span(
            11,
            SpanKind::Server,
            "POST /orders",
            started_at,
            50,
            &json!({}),
        ),
    ]
}

struct Fixture {
    directory: tempfile::TempDir,
    today_start_at: i64,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let today_start_at = Day::today().start_at();
        let started_at = today_start_at + SECOND;
        let api = records("api", api_spans(started_at));
        let worker = records(
            "worker",
            vec![child_span(
                12,
                SpanKind::Client,
                "SELECT",
                started_at,
                10,
                &postgres_attributes("SELECT * FROM users WHERE id = 3"),
            )],
        );
        write_batch(directory.path(), vec![api, worker]);
        Self {
            directory,
            today_start_at,
        }
    }

    fn reader(&self) -> Reader {
        let range = TimeRange::new(self.today_start_at, self.today_start_at + 3 * MINUTE).unwrap();
        Reader::open(self.directory.path(), range).unwrap()
    }
}

const USERS_QUERY_TEMPLATE: &str = "SELECT * FROM users WHERE id = ?";

#[test]
fn calls_go_by_target_the_most_time_first() {
    let fixture = Fixture::new();
    let calls = fixture.reader().list_calls("api", MINUTE, 10).unwrap();
    assert_eq!((calls.calls.count, calls.calls.errors), (8, 1));
    assert_eq!(calls.calls.total_ns, 96 * MILLISECOND);
    let counts: Vec<_> = calls
        .buckets
        .iter()
        .map(|bucket| bucket.requests.count)
        .collect();
    assert_eq!(counts, [7, 1, 0]);
    assert!(!calls.truncated);

    let targets: Vec<_> = calls
        .targets
        .iter()
        .map(|target| {
            (
                target.key.clone(),
                target.query.as_str(),
                target.calls.count,
            )
        })
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

    let summaries = |target_index: usize| -> Vec<_> {
        calls.targets[target_index]
            .operations
            .iter()
            .map(|operation| (operation.summary.as_str(), operation.calls.count))
            .collect()
    };
    assert_eq!(
        summaries(0),
        [("GET /v1/customers/{id}", 1), ("GET /v1/prices/{id}", 1)]
    );
    assert_eq!(
        summaries(1),
        [
            (USERS_QUERY_TEMPLATE, 2),
            ("SELECT * FROM orders WHERE id IN (?) AND note = ?", 1)
        ]
    );
    assert_eq!(summaries(2), [("POST /hook", 1)]);
    assert_eq!(summaries(3), [("SELECT users", 1)]);
    assert_eq!(summaries(4), [("orders publish", 1)]);

    let postgres = &calls.targets[1];
    let counts: Vec<_> = postgres
        .buckets
        .iter()
        .map(|bucket| bucket.requests.count)
        .collect();
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
    let calls = fixture.reader().list_calls("api", MINUTE, 2).unwrap();
    assert!(calls.truncated);
    assert_eq!(calls.targets.len(), 5);
    let kept: Vec<_> = calls
        .targets
        .iter()
        .map(|target| target.operations.len())
        .collect();
    assert_eq!(kept, [1, 1, 0, 0, 0]);

    let unknown_service_calls = fixture.reader().list_calls("nobody", MINUTE, 10).unwrap();
    assert_eq!(unknown_service_calls.calls.count, 0);
    assert!(unknown_service_calls.targets.is_empty());
    assert_eq!(unknown_service_calls.buckets.len(), 3);
}

#[test]
fn a_call_is_what_it_does_to_one_target() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let postgres = target_key(TargetType::Database, Some("postgresql"), Some("app"));
    let users = reader
        .get_call(
            "api",
            &postgres,
            USERS_QUERY_TEMPLATE,
            SpanKind::Client,
            MINUTE,
        )
        .unwrap();
    let operation = &users.operation;
    assert_eq!(
        (operation.requests.count, operation.requests.errors),
        (2, 1)
    );
    let counts: Vec<_> = operation
        .buckets
        .iter()
        .map(|bucket| bucket.requests.count)
        .collect();
    assert_eq!(counts, [1, 1, 0]);
    assert_eq!(operation.name, "SELECT");
    assert_eq!(operation.attributes["newest"], "yes");
    assert_eq!(
        users.query,
        r#"db.system.name = "postgresql" db.namespace = "app" name = "SELECT""#
    );
    let ids: Vec<_> = users.spans.iter().map(|span| span.span_id).collect();
    assert_eq!(ids, [SpanId([3; 8]), SpanId([2; 8])]);

    let stripe = target_key(TargetType::Http, None, Some("api.stripe.com"));
    let prices = reader
        .get_call(
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
    let internal_kind_call = reader
        .get_call("api", &sqlite, "SELECT users", SpanKind::Internal, MINUTE)
        .unwrap();
    assert_eq!(internal_kind_call.operation.requests.count, 1);
    let client_kind_call = reader
        .get_call("api", &sqlite, "SELECT users", SpanKind::Client, MINUTE)
        .unwrap();
    assert_eq!(client_kind_call.operation.requests.count, 0);
    assert!(client_kind_call.operation.attributes.is_empty());
    assert!(client_kind_call.spans.is_empty());
    assert_eq!(client_kind_call.query, "");
}
