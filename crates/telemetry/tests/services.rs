use std::path::Path;

use serde_json::{Value, json};
use siner_telemetry::query::Latency;
use siner_telemetry::{
    Attributes, Batch, Config, Day, Log, Reader, Records, Resource, Span, Writer, channel,
};

/// Attributes from a JSON object, as the day files keep them.
fn attrs(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

const SECOND: i64 = 1_000_000_000;
const MS: i64 = 1_000_000;
const MINUTE: i64 = 60 * SECOND;

fn records(service: &str, attributes: &Value) -> Records {
    Records {
        resource: Resource {
            service: service.into(),
            attributes: attrs(attributes.clone()),
        },
        logs: Vec::new(),
        spans: Vec::new(),
        metrics: Vec::new(),
    }
}

/// A span of `trace`, with the parent `parent` when it has one.
fn span(trace: u8, id: u8, parent: Option<u8>, kind: i32, name: &str, start: i64) -> Span {
    Span {
        trace_id: [trace; 16],
        span_id: [id; 8],
        parent_span_id: parent.map(|p| [p; 8]),
        name: name.into(),
        kind,
        start_ts: start,
        duration_ns: 10 * MS,
        status: 0,
        attributes: Attributes::new(),
        events: Vec::new(),
    }
}

fn log(ts: i64, severity: i32) -> Log {
    Log {
        ts,
        severity,
        body: "a line".into(),
        trace_id: None,
        span_id: None,
        attributes: Attributes::new(),
        source: "otlp",
    }
}

fn write(dir: &Path, batch: Batch) {
    let (sender, inbox) = channel(1);
    assert!(sender.send(batch));
    let writer = Writer::spawn(Config::new(dir.to_owned()), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

/// Telemetry from the last minute of yesterday and the first minutes of
/// today. `start` is the first nanosecond of today.
struct Fixture {
    dir: tempfile::TempDir,
    start: i64,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let start = Day::today().start();
        let y = start - 30 * SECOND;

        let mut old_api = records("api", &json!({"telemetry.sdk.language": "go"}));
        old_api.spans = vec![Span {
            attributes: attrs(
                json!({"http.route": "/users", "http.request.method": "GET", "old": true}),
            ),
            ..span(1, 1, None, 2, "GET /users", y)
        }];
        old_api.logs = vec![log(y, 17)];

        let mut api = records("api", &json!({"telemetry.sdk.language": "rust"}));
        api.spans = vec![
            // A request, the database call it makes, and a job it queues.
            Span {
                duration_ns: 30 * MS,
                status: 2,
                attributes: attrs(json!({"http.route": "/users", "http.request.method": "GET"})),
                ..span(2, 1, None, 2, "GET /users", start + SECOND)
            },
            span(2, 2, Some(1), 3, "SELECT users", start + SECOND),
            span(2, 3, Some(1), 4, "send email", start + SECOND),
            // A server span under a span of another service counts too.
            Span {
                duration_ns: 20 * MS,
                ..span(3, 2, Some(1), 2, "POST /orders", start + MINUTE + SECOND)
            },
        ];
        api.logs = vec![log(start + SECOND, 9), log(start + MINUTE, 13)];

        let mut worker = records("worker", &json!({}));
        worker.spans = vec![
            span(2, 4, Some(3), 5, "send email", start + 2 * SECOND),
            span(2, 5, Some(4), 1, "render", start + 2 * SECOND),
        ];

        // A resource without attributes, newer than the one of `api`.
        let mut bare_api = records("api", &json!({}));
        bare_api.logs = vec![log(start + 2 * MINUTE, 9)];

        let mut cron = records("cron", &json!({}));
        cron.logs = vec![log(start + SECOND, 9), log(start + SECOND, 21)];

        write(dir.path(), vec![old_api, api, worker, cron, bare_api]);
        Self { dir, start }
    }

    fn reader(&self) -> Reader {
        Reader::open(
            self.dir.path(),
            self.start - MINUTE,
            self.start + 3 * MINUTE,
        )
        .unwrap()
    }
}

#[test]
fn services_count_the_spans_that_enter_them_and_their_logs() {
    let fixture = Fixture::new();
    let list = fixture.reader().services(MINUTE, 10).unwrap();
    assert_eq!(list.step_ns, MINUTE);
    assert!(!list.truncated);
    let names: Vec<_> = list.services.iter().map(|s| s.service.as_str()).collect();
    // The most requests first, then the most logs.
    assert_eq!(names, ["api", "worker", "cron"]);

    let api = &list.services[0];
    // The newest resource of the service.
    assert_eq!(
        api.resource,
        attrs(json!({"telemetry.sdk.language": "rust"}))
    );
    let requests = &api.stats.requests;
    assert_eq!((requests.count, requests.errors), (3, 1));
    assert_eq!(requests.total_ns, 60 * MS);
    let latency = requests.latency.unwrap();
    assert!((latency.p50 - 20 * MS).abs() <= MS / 5, "{latency:?}");
    assert!((latency.p99 - 30 * MS).abs() <= 3 * MS / 10, "{latency:?}");
    assert_eq!((api.stats.logs, api.stats.error_logs), (4, 1));

    // A bucket for every minute of the range, empty or not.
    assert_eq!(api.buckets.len(), 4);
    let counts: Vec<_> = api.buckets.iter().map(|b| b.requests.count).collect();
    assert_eq!(counts, [1, 1, 1, 0]);
    let logs: Vec<_> = api.buckets.iter().map(|b| b.logs).collect();
    assert_eq!(logs, [1, 1, 1, 1]);
    assert_eq!(api.buckets[3].requests.latency, None);

    let worker = &list.services[1];
    assert_eq!(worker.stats.requests.count, 1);
    assert!(worker.resource.is_empty());

    let cron = &list.services[2];
    assert_eq!(cron.stats.requests.count, 0);
    assert_eq!(cron.stats.requests.latency, None);
    assert_eq!((cron.stats.logs, cron.stats.error_logs), (2, 1));

    let cut = fixture.reader().services(MINUTE, 1).unwrap();
    assert_eq!(cut.services.len(), 1);
    assert!(cut.truncated);
}

#[test]
fn a_service_has_its_requests_by_operation() {
    let fixture = Fixture::new();
    let api = fixture.reader().service("api", MINUTE, 10).unwrap();
    assert_eq!(api.stats.requests.count, 3);
    assert_eq!(api.buckets.len(), 4);
    let operations: Vec<_> = api
        .operations
        .iter()
        .map(|o| (o.name.as_str(), o.kind, o.requests.count, o.requests.errors))
        .collect();
    assert_eq!(
        operations,
        [("GET /users", 2, 2, 1), ("POST /orders", 2, 1, 0)]
    );
    // The attributes of the newest request, from today's file.
    assert_eq!(
        api.operations[0].attributes,
        attrs(json!({"http.route": "/users", "http.request.method": "GET"}))
    );
    assert!(api.operations[1].attributes.is_empty());

    // One request is every percentile.
    let Latency { p50, p95, p99 } = api.operations[1].requests.latency.unwrap();
    assert!((p50 - 20 * MS).abs() <= MS / 5, "{p50}");
    assert_eq!((p95, p99), (p50, p50));

    let cut = fixture.reader().service("api", MINUTE, 1).unwrap();
    assert_eq!(cut.operations.len(), 1);
    assert!(cut.truncated);

    let none = fixture.reader().service("nobody", MINUTE, 10).unwrap();
    assert_eq!(none.stats.requests.count, 0);
    assert_eq!(none.stats.logs, 0);
    assert!(none.resource.is_empty());
    assert!(none.operations.is_empty());
    assert!(none.buckets.iter().all(|b| b.requests.count == 0));
}

#[test]
fn the_step_has_to_fit_the_range() {
    let fixture = Fixture::new();
    assert!(fixture.reader().services(0, 10).is_err());
    assert!(fixture.reader().service("api", 1, 10).is_err());
}

#[test]
fn percentiles_stay_within_a_percent_of_the_values() {
    let dir = tempfile::tempdir().unwrap();
    let start = Day::today().start();
    let mut api = records("api", &json!({}));
    // Durations of 1 ms to 1000 ms, one each.
    api.spans = (1..=1000)
        .map(|i| Span {
            trace_id: [0; 16],
            span_id: i64::to_be_bytes(i),
            duration_ns: i * MS,
            ..span(0, 0, None, 2, "GET /", start + SECOND)
        })
        .collect();
    write(dir.path(), vec![api]);
    let reader = Reader::open(dir.path(), start, start + MINUTE).unwrap();
    let latency = reader.services(MINUTE, 10).unwrap().services[0]
        .stats
        .requests
        .latency
        .unwrap();
    for (estimate, exact) in [
        (latency.p50, 500 * MS),
        (latency.p95, 950 * MS),
        (latency.p99, 990 * MS),
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
        .operation("api", "GET /users", 2, MINUTE)
        .unwrap();
    assert_eq!((users.requests.count, users.requests.errors), (2, 1));
    let counts: Vec<_> = users.buckets.iter().map(|b| b.requests.count).collect();
    assert_eq!(counts, [1, 1, 0, 0]);
    assert_eq!(users.attributes["http.route"], "/users");
    assert!(users.attributes.get("old").is_none());

    // The name and the kind together name an operation.
    let other_kind = fixture
        .reader()
        .operation("api", "GET /users", 5, MINUTE)
        .unwrap();
    assert_eq!(other_kind.requests.count, 0);
    assert!(other_kind.attributes.is_empty());
    assert_eq!(other_kind.buckets.len(), 4);
}
