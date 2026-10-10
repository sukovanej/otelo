mod common;

use std::path::Path;

use otelo_indexed_storage::{
    Attributes, Batch, Log, Metric, NumberPoint, Points, RangeQueries, Records, Resource, Severity,
    Span, SpanId, SpanKind, SpanStatus, TimeRange, TraceContext, TraceId,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader};
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
        status_code: SpanStatus::Unset,
        attributes: Attributes::new(),
        events: Vec::new(),
    }
}

fn spans_of_no_request(started_at: i64) -> [Span; 2] {
    [
        span(4, 1, None, SpanKind::Server, "health check", started_at),
        span(5, 1, None, SpanKind::Client, "SELECT", started_at),
    ]
}

fn log(logged_at: i64, severity: Severity) -> Log {
    Log {
        logged_at,
        severity_number: severity,
        body: "a line".into(),
        trace_context: TraceContext::None,
        attributes: Attributes::new(),
    }
}

fn write_batch(directory: &Path, batch: Batch) {
    common::index_batches(Config::new(directory.to_owned()), vec![batch]);
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
                status_code: SpanStatus::Error,
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
                attributes: attributes_from_json(json!({"http.request.method": "POST"})),
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
        api.spans
            .extend(spans_of_no_request(today_start_at + MINUTE + SECOND));
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
fn services_count_their_http_requests_and_their_logs() {
    let fixture = Fixture::new();
    let service_list = fixture.reader().list_services(MINUTE, 10).unwrap();
    assert_eq!(service_list.step_ns, MINUTE);
    assert!(!service_list.truncated);
    let names: Vec<_> = service_list
        .services
        .iter()
        .map(|summary| summary.service.as_str())
        .collect();
    assert_eq!(names, ["api", "cron", "worker"]);

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
    assert_eq!(api.stats.spans, 7);
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

    let worker = &service_list.services[2];
    assert_eq!(worker.stats.requests.count, 0);
    assert_eq!(worker.stats.spans, 2);
    assert!(worker.resource.is_empty());

    let cron = &service_list.services[1];
    assert_eq!(cron.stats.requests.count, 0);
    assert_eq!(cron.stats.requests.latency, None);
    assert_eq!((cron.stats.logs, cron.stats.error_logs), (2, 1));

    let truncated_service_list = fixture.reader().list_services(MINUTE, 1).unwrap();
    assert_eq!(truncated_service_list.services.len(), 1);
    assert!(truncated_service_list.truncated);
}

#[test]
fn a_service_has_its_requests_over_time() {
    let fixture = Fixture::new();
    let api = fixture.reader().get_service("api", MINUTE).unwrap();
    assert_eq!(api.stats.requests.count, 3);
    assert_eq!(api.stats.logs, 4);
    let counts: Vec<_> = api
        .buckets
        .iter()
        .map(|bucket| bucket.requests.count)
        .collect();
    assert_eq!(counts, [1, 1, 1, 0]);
    assert_eq!(api.resource["telemetry.sdk.language"], "rust");

    let unknown_service = fixture.reader().get_service("nobody", MINUTE).unwrap();
    assert_eq!(unknown_service.stats.requests.count, 0);
    assert_eq!(unknown_service.stats.logs, 0);
    assert!(unknown_service.resource.is_empty());
    assert!(
        unknown_service
            .buckets
            .iter()
            .all(|bucket| bucket.requests.count == 0)
    );
}

#[test]
fn a_service_has_the_resource_of_its_newest_log_or_span() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();

    let mut new_sdk = records("api", &json!({"telemetry.sdk.language": "rust"}));
    new_sdk.spans = vec![span(
        1,
        1,
        None,
        SpanKind::Internal,
        "tick",
        today_start_at + 2 * SECOND,
    )];
    let mut old_sdk = records("api", &json!({"telemetry.sdk.language": "go"}));
    old_sdk.logs = vec![log(today_start_at + SECOND, Severity::INFO)];
    let mut host = records("api", &json!({"host.name": "server"}));
    host.metrics = vec![Metric {
        name: "process.memory.usage".into(),
        unit: "By".into(),
        attributes: Attributes::new(),
        points: Points::UpDown(vec![NumberPoint {
            recorded_at: today_start_at + 3 * SECOND,
            value: 1.0,
        }]),
    }];
    write_batch(directory.path(), vec![new_sdk, old_sdk, host]);

    let reader = Reader::open(
        directory.path(),
        TimeRange::new(today_start_at, today_start_at + MINUTE).unwrap(),
    )
    .unwrap();
    let rust = attributes_from_json(json!({"telemetry.sdk.language": "rust"}));
    assert_eq!(
        reader.list_services(MINUTE, 10).unwrap().services[0].resource,
        rust
    );
    assert_eq!(reader.get_service("api", MINUTE).unwrap().resource, rust);
}

#[test]
fn the_step_has_to_fit_the_range() {
    let fixture = Fixture::new();
    assert!(fixture.reader().list_services(0, 10).is_err());
    assert!(fixture.reader().get_service("api", 1).is_err());
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
            attributes: attributes_from_json(json!({"http.request.method": "GET"})),
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
