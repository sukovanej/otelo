use std::path::Path;

use otelo_indexed_storage::query::Latency;
use otelo_indexed_storage::{
    Attributes, Batch, Log, LogSource, RangeQueries, Records, Resource, Severity, Span, SpanId,
    SpanKind, SpanStatus, TimeRange, TraceContext, TraceId, open_batch_channel,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader, Writer};
use serde_json::{Value, json};

fn attributes_from_json(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

const SECOND: i64 = 1_000_000_000;
const MILLISECOND: i64 = 1_000_000;
const MINUTE: i64 = 60 * SECOND;

fn records(service: &str, attributes: &Value) -> Records {
    Records {
        resource: Resource {
            service: service.into(),
            attributes: attributes_from_json(attributes.clone()),
        },
        logs: Vec::new(),
        spans: Vec::new(),
        metrics: Vec::new(),
    }
}

fn span(
    trace_id_byte: u8,
    span_id_byte: u8,
    parent_span_id_byte: Option<u8>,
    kind: SpanKind,
    name: &str,
    started_at: i64,
) -> Span {
    Span {
        trace_id: TraceId([trace_id_byte; 16]),
        span_id: SpanId([span_id_byte; 8]),
        parent_span_id: parent_span_id_byte.map(|byte| SpanId([byte; 8])),
        name: name.into(),
        kind,
        started_at,
        duration_ns: 10 * MILLISECOND,
        status: SpanStatus::Unset,
        attributes: Attributes::new(),
        events: Vec::new(),
    }
}

fn log(logged_at: i64, severity: Severity) -> Log {
    Log {
        logged_at,
        severity,
        body: "a line".into(),
        trace_context: TraceContext::None,
        attributes: Attributes::new(),
        source: LogSource::Otlp,
    }
}

fn write_batch(directory: &Path, batch: Batch) {
    let (sender, inbox) = open_batch_channel(1);
    assert!(sender.send_batch(batch));
    let writer = Writer::spawn(Config::new(directory.to_owned()), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

struct Fixture {
    directory: tempfile::TempDir,
    today_start_at: i64,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let today_start_at = Day::today().start_at();
        let yesterday_at = today_start_at - 30 * SECOND;

        let mut old_api = records("api", &json!({"telemetry.sdk.language": "go"}));
        old_api.spans = vec![Span {
            attributes: attributes_from_json(
                json!({"http.route": "/users", "http.request.method": "GET", "old": true}),
            ),
            ..span(1, 1, None, SpanKind::Server, "GET /users", yesterday_at)
        }];
        old_api.logs = vec![log(yesterday_at, Severity::ERROR)];

        let mut api = records("api", &json!({"telemetry.sdk.language": "rust"}));
        api.spans = vec![
            Span {
                duration_ns: 30 * MILLISECOND,
                status: SpanStatus::Error,
                attributes: attributes_from_json(
                    json!({"http.route": "/users", "http.request.method": "GET"}),
                ),
                ..span(
                    2,
                    1,
                    None,
                    SpanKind::Server,
                    "GET /users",
                    today_start_at + SECOND,
                )
            },
            span(
                2,
                2,
                Some(1),
                SpanKind::Client,
                "SELECT users",
                today_start_at + SECOND,
            ),
            span(
                2,
                3,
                Some(1),
                SpanKind::Producer,
                "send email",
                today_start_at + SECOND,
            ),
            Span {
                duration_ns: 20 * MILLISECOND,
                ..span(
                    3,
                    2,
                    Some(1),
                    SpanKind::Server,
                    "POST /orders",
                    today_start_at + MINUTE + SECOND,
                )
            },
        ];
        api.logs = vec![
            log(today_start_at + SECOND, Severity::INFO),
            log(today_start_at + MINUTE, Severity::WARN),
        ];

        let mut worker = records("worker", &json!({}));
        worker.spans = vec![
            span(
                2,
                4,
                Some(3),
                SpanKind::Consumer,
                "send email",
                today_start_at + 2 * SECOND,
            ),
            span(
                2,
                5,
                Some(4),
                SpanKind::Internal,
                "render",
                today_start_at + 2 * SECOND,
            ),
        ];

        let mut bare_api = records("api", &json!({}));
        bare_api.logs = vec![log(today_start_at + 2 * MINUTE, Severity::INFO)];

        let mut cron = records("cron", &json!({}));
        cron.logs = vec![
            log(today_start_at + SECOND, Severity::INFO),
            log(today_start_at + SECOND, Severity::FATAL),
        ];

        write_batch(directory.path(), vec![old_api, api, worker, cron, bare_api]);
        Self {
            directory,
            today_start_at,
        }
    }

    fn reader(&self) -> Reader {
        Reader::open(
            self.directory.path(),
            TimeRange::new(
                self.today_start_at - MINUTE,
                self.today_start_at + 3 * MINUTE,
            )
            .unwrap(),
        )
        .unwrap()
    }
}

#[test]
fn services_count_the_spans_that_enter_them_and_their_logs() {
    let fixture = Fixture::new();
    let service_list = fixture.reader().list_services(MINUTE, 10).unwrap();
    assert_eq!(service_list.step_ns, MINUTE);
    assert!(!service_list.truncated);
    let names: Vec<_> = service_list
        .services
        .iter()
        .map(|summary| summary.service.as_str())
        .collect();
    assert_eq!(names, ["api", "worker", "cron"]);

    let api = &service_list.services[0];
    assert_eq!(
        api.resource,
        attributes_from_json(json!({"telemetry.sdk.language": "rust"}))
    );
    let requests = &api.stats.requests;
    assert_eq!((requests.count, requests.errors), (3, 1));
    assert_eq!(requests.total_ns, 60 * MILLISECOND);
    let latency = requests.latency.unwrap();
    assert!(
        (latency.p50 - 20 * MILLISECOND).abs() <= MILLISECOND / 5,
        "{latency:?}"
    );
    assert!(
        (latency.p99 - 30 * MILLISECOND).abs() <= 3 * MILLISECOND / 10,
        "{latency:?}"
    );
    assert_eq!((api.stats.logs, api.stats.error_logs), (4, 1));

    assert_eq!(api.buckets.len(), 4);
    let counts: Vec<_> = api
        .buckets
        .iter()
        .map(|bucket| bucket.requests.count)
        .collect();
    assert_eq!(counts, [1, 1, 1, 0]);
    let logs: Vec<_> = api.buckets.iter().map(|bucket| bucket.logs).collect();
    assert_eq!(logs, [1, 1, 1, 1]);
    assert_eq!(api.buckets[3].requests.latency, None);

    let worker = &service_list.services[1];
    assert_eq!(worker.stats.requests.count, 1);
    assert!(worker.resource.is_empty());

    let cron = &service_list.services[2];
    assert_eq!(cron.stats.requests.count, 0);
    assert_eq!(cron.stats.requests.latency, None);
    assert_eq!((cron.stats.logs, cron.stats.error_logs), (2, 1));

    let truncated_service_list = fixture.reader().list_services(MINUTE, 1).unwrap();
    assert_eq!(truncated_service_list.services.len(), 1);
    assert!(truncated_service_list.truncated);
}

#[test]
fn a_service_has_its_requests_by_operation() {
    let fixture = Fixture::new();
    let api = fixture.reader().get_service("api", MINUTE, 10).unwrap();
    assert_eq!(api.stats.requests.count, 3);
    assert_eq!(api.buckets.len(), 4);
    let operations: Vec<_> = api
        .operations
        .iter()
        .map(|operation| {
            (
                operation.name.as_str(),
                operation.kind,
                operation.requests.count,
                operation.requests.errors,
            )
        })
        .collect();
    assert_eq!(
        operations,
        [
            ("GET /users", SpanKind::Server, 2, 1),
            ("POST /orders", SpanKind::Server, 1, 0)
        ]
    );
    assert_eq!(
        api.operations[0].attributes,
        attributes_from_json(json!({"http.route": "/users", "http.request.method": "GET"}))
    );
    assert!(api.operations[1].attributes.is_empty());

    let Latency { p50, p95, p99 } = api.operations[1].requests.latency.unwrap();
    assert!((p50 - 20 * MILLISECOND).abs() <= MILLISECOND / 5, "{p50}");
    assert_eq!((p95, p99), (p50, p50));

    let truncated_service = fixture.reader().get_service("api", MINUTE, 1).unwrap();
    assert_eq!(truncated_service.operations.len(), 1);
    assert!(truncated_service.truncated);

    let unknown_service = fixture.reader().get_service("nobody", MINUTE, 10).unwrap();
    assert_eq!(unknown_service.stats.requests.count, 0);
    assert_eq!(unknown_service.stats.logs, 0);
    assert!(unknown_service.resource.is_empty());
    assert!(unknown_service.operations.is_empty());
    assert!(
        unknown_service
            .buckets
            .iter()
            .all(|bucket| bucket.requests.count == 0)
    );
}

#[test]
fn the_step_has_to_fit_the_range() {
    let fixture = Fixture::new();
    assert!(fixture.reader().list_services(0, 10).is_err());
    assert!(fixture.reader().get_service("api", 1, 10).is_err());
}

#[test]
fn percentiles_stay_within_a_percent_of_the_values() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    let mut api = records("api", &json!({}));
    api.spans = (1..=1000)
        .map(|request_number| Span {
            trace_id: TraceId([0; 16]),
            span_id: SpanId(i64::to_be_bytes(request_number)),
            duration_ns: request_number * MILLISECOND,
            ..span(
                0,
                0,
                None,
                SpanKind::Server,
                "GET /",
                today_start_at + SECOND,
            )
        })
        .collect();
    write_batch(directory.path(), vec![api]);
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(today_start_at, today_start_at + MINUTE).unwrap(),
    )
    .unwrap();
    let latency = reader.list_services(MINUTE, 10).unwrap().services[0]
        .stats
        .requests
        .latency
        .unwrap();
    for (estimate, exact) in [
        (latency.p50, 500 * MILLISECOND),
        (latency.p95, 950 * MILLISECOND),
        (latency.p99, 990 * MILLISECOND),
    ] {
        assert!(
            (estimate - exact).abs() <= exact / 100,
            "{estimate} for {exact}"
        );
    }
}

#[test]
fn an_operation_has_its_requests_over_time() {
    let fixture = Fixture::new();
    let users = fixture
        .reader()
        .get_operation("api", "GET /users", SpanKind::Server, MINUTE)
        .unwrap();
    assert_eq!((users.requests.count, users.requests.errors), (2, 1));
    let counts: Vec<_> = users
        .buckets
        .iter()
        .map(|bucket| bucket.requests.count)
        .collect();
    assert_eq!(counts, [1, 1, 0, 0]);
    assert_eq!(users.attributes["http.route"], "/users");
    assert!(users.attributes.get("old").is_none());

    let other_kind = fixture
        .reader()
        .get_operation("api", "GET /users", SpanKind::Consumer, MINUTE)
        .unwrap();
    assert_eq!(other_kind.requests.count, 0);
    assert!(other_kind.attributes.is_empty());
    assert_eq!(other_kind.buckets.len(), 4);
}
