use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value, json};
use siner_telemetry::query::{LogFilter, MetricFilter, TraceFilter};
use siner_telemetry::{
    Batch, Config, Day, Log, Metric, MetricKind, Point, Reader, Records, Resource, Span, Writer,
    channel, timed_out,
};

const SECOND: i64 = 1_000_000_000;
const TRACE: [u8; 16] = [0xab; 16];
const TRACE_HEX: &str = "abababababababababababababababab";

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn resource(service: &str) -> Resource {
    Resource {
        service: service.into(),
        attributes: object(&json!({"service.name": service})),
    }
}

fn log(ts: i64, severity: i32, body: &str) -> Log {
    Log {
        ts,
        severity,
        body: body.into(),
        trace_id: None,
        span_id: None,
        attributes: Map::new(),
        source: "otlp",
    }
}

fn span(trace: [u8; 16], id: u8, parent: Option<u8>, start: i64, name: &str) -> Span {
    Span {
        trace_id: trace,
        span_id: [id; 8],
        parent_span_id: parent.map(|p| [p; 8]),
        name: name.into(),
        kind: 2,
        start_ts: start,
        duration_ns: 10_000_000,
        status: 0,
        attributes: Map::new(),
        events: Vec::new(),
    }
}

fn records(service: &str) -> Records {
    Records {
        resource: resource(service),
        logs: Vec::new(),
        spans: Vec::new(),
        metrics: Vec::new(),
    }
}

fn write(dir: &Path, batch: Batch) {
    let (sender, inbox) = channel(1);
    sender.send(batch);
    let writer = Writer::spawn(Config::new(dir.to_owned()), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

/// A day file for yesterday and one for today. `start` is the first
/// nanosecond of today.
struct Fixture {
    dir: tempfile::TempDir,
    start: i64,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let start = Day::today().start();
        let (y, t) = (start - 60 * SECOND, start + SECOND);

        let mut api = records("api");
        api.logs = vec![
            log(y, 9, "user 7 signed in"),
            log(t, 9, "user 8 signed in"),
            log(t + SECOND, 17, "payment 12 failed: card \"visa\" declined"),
            log(
                t + 2 * SECOND,
                17,
                "payment 13 failed: card \"amex\" declined",
            ),
            Log {
                trace_id: Some(TRACE),
                span_id: Some([2; 8]),
                ..log(t + 3 * SECOND, 13, "slow query GET /languages")
            },
        ];
        let mut failed = span(TRACE, 2, Some(1), t + 2 * SECOND, "SELECT languages");
        failed.status = 2;
        let mut root = span(TRACE, 1, None, t, "GET /languages");
        root.duration_ns = 900_000_000;
        api.spans = vec![
            root,
            failed,
            span([0xcd; 16], 1, None, y, "GET /health"),
            span([0xef; 16], 1, None, t + SECOND, "POST /matches"),
        ];
        api.metrics = vec![Metric {
            name: "process.memory.usage".into(),
            kind: MetricKind::Gauge,
            unit: "By".into(),
            labels: object(&json!({"state": "used"})),
            points: [
                (t, 100.0),
                (t + 10 * SECOND, 300.0),
                (t + 70 * SECOND, 50.0),
            ]
            .into_iter()
            .map(|(ts, value)| Point {
                ts,
                value,
                histogram: None,
            })
            .collect(),
        }];
        let mut caddy = records("caddy");
        caddy.logs = vec![log(t, 9, "served 200 in 3ms")];
        write(dir.path(), vec![api, caddy]);
        Self { dir, start }
    }

    /// A reader of the last day file and the first minutes of today.
    fn reader(&self) -> Reader {
        Reader::open(
            self.dir.path(),
            self.start - 3600 * SECOND,
            self.start + 600 * SECOND,
        )
        .unwrap()
    }
}

fn bodies(logs: &siner_telemetry::query::Logs) -> Vec<&str> {
    logs.logs.iter().map(|line| line.body.as_str()).collect()
}

#[test]
fn logs_come_newest_first_and_say_when_they_are_cut() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let logs = reader.logs(&LogFilter::default(), 3).unwrap();
    assert!(logs.truncated);
    assert_eq!(
        bodies(&logs),
        [
            "slow query GET /languages",
            "payment 13 failed: card \"amex\" declined",
            "payment 12 failed: card \"visa\" declined",
        ]
    );
    assert_eq!(logs.logs[0].level, "WARN");
    assert_eq!(logs.logs[0].trace_id.as_deref(), Some(TRACE_HEX));
    assert!(!reader.logs(&LogFilter::default(), 7).unwrap().truncated);
}

#[test]
fn logs_filter_by_service_severity_trace_and_words_across_days() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let filter = |f: LogFilter| bodies(&reader.logs(&f, 10).unwrap()).join(" | ");
    assert_eq!(
        filter(LogFilter {
            service: Some("caddy".into()),
            ..LogFilter::default()
        }),
        "served 200 in 3ms"
    );
    assert_eq!(
        filter(LogFilter {
            min_severity: Some(17),
            service: Some("api".into()),
            ..LogFilter::default()
        }),
        "payment 13 failed: card \"amex\" declined | payment 12 failed: card \"visa\" declined"
    );
    assert_eq!(
        filter(LogFilter {
            trace_id: Some(TRACE),
            ..LogFilter::default()
        }),
        "slow query GET /languages"
    );
    // The words are found in both day files, and punctuation is no syntax.
    assert_eq!(
        filter(LogFilter {
            search: Some("signed IN".into()),
            ..LogFilter::default()
        }),
        "user 8 signed in | user 7 signed in"
    );
    assert_eq!(
        filter(LogFilter {
            search: Some("GET /languages".into()),
            ..LogFilter::default()
        }),
        "slow query GET /languages"
    );
}

#[test]
fn log_groups_count_lines_by_template() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let groups = reader.log_groups(&LogFilter::default(), 2).unwrap();
    assert!(groups.truncated);
    assert!(!groups.partial);
    assert_eq!(groups.scanned, 6);
    let found: Vec<(&str, u64)> = groups
        .groups
        .iter()
        .map(|g| (g.template.as_str(), g.count))
        .collect();
    assert_eq!(
        found,
        [
            ("payment <num> failed: card \"<str>\" declined", 2),
            ("user <num> signed in", 2),
        ]
    );
    let payment = &groups.groups[0];
    assert_eq!(payment.level, "ERROR");
    assert_eq!(payment.services, ["api"]);
    assert_eq!(
        payment.samples,
        [
            "payment 13 failed: card \"amex\" declined",
            "payment 12 failed: card \"visa\" declined"
        ]
    );
    assert!(payment.first < payment.last);
}

#[test]
fn traces_list_roots_with_span_counts_and_errors() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let traces = reader.traces(&TraceFilter::default(), 10).unwrap();
    let found: Vec<(&str, u64, bool)> = traces
        .traces
        .iter()
        .map(|t| (t.name.as_str(), t.spans, t.error))
        .collect();
    assert_eq!(
        found,
        [
            ("POST /matches", 1, false),
            ("GET /languages", 2, true),
            ("GET /health", 1, false),
        ]
    );
    assert!(!traces.truncated);
    assert!(reader.traces(&TraceFilter::default(), 2).unwrap().truncated);

    let names = |filter: TraceFilter| -> Vec<String> {
        let traces = reader.traces(&filter, 10).unwrap().traces;
        traces.into_iter().map(|t| t.name).collect()
    };
    let errors = TraceFilter {
        errors: true,
        ..TraceFilter::default()
    };
    assert_eq!(names(errors), ["GET /languages"]);
    let slow = TraceFilter {
        min_duration_ns: Some(500_000_000),
        ..TraceFilter::default()
    };
    assert_eq!(names(slow), ["GET /languages"]);
    let get = TraceFilter {
        name: Some("GET".into()),
        ..TraceFilter::default()
    };
    assert_eq!(names(get), ["GET /languages", "GET /health"]);
}

#[test]
fn a_trace_has_its_spans_and_logs() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let trace = reader.trace(TRACE, 10).unwrap().unwrap();
    assert_eq!(trace.trace_id, TRACE_HEX);
    let spans: Vec<(&str, Option<&str>, bool)> = trace
        .spans
        .iter()
        .map(|s| (s.name.as_str(), s.parent_span_id.as_deref(), s.error))
        .collect();
    assert_eq!(
        spans,
        [
            ("GET /languages", None, false),
            ("SELECT languages", Some("0101010101010101"), true)
        ]
    );
    assert_eq!(trace.logs.len(), 1);
    assert!(reader.trace([0x99; 16], 10).unwrap().is_none());
}

#[test]
fn metrics_list_series_and_bucket_points() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let list = reader.metrics(None, 10).unwrap();
    let names: Vec<(&str, &str)> = list
        .series
        .iter()
        .map(|s| (s.name.as_str(), s.service.as_str()))
        .collect();
    assert!(
        names.contains(&("process.memory.usage", "api")),
        "{names:?}"
    );
    assert!(reader.metrics(Some("caddy"), 10).unwrap().series.is_empty());

    let filter = MetricFilter {
        name: "process.memory.usage".into(),
        service: None,
        labels: vec![("state".into(), "used".into())],
        step_ns: 60 * SECOND,
    };
    let metric = reader.metric(&filter, 10).unwrap();
    assert_eq!(metric.series.len(), 1);
    let buckets: Vec<(u64, f64, f64, f64, f64)> = metric.series[0]
        .buckets
        .iter()
        .map(|b| (b.count, b.min, b.max, b.avg, b.last))
        .collect();
    assert_eq!(
        buckets,
        [(2, 100.0, 300.0, 200.0, 300.0), (1, 50.0, 50.0, 50.0, 50.0)]
    );
    let free = MetricFilter {
        labels: vec![("state".into(), "free".into())],
        ..filter
    };
    assert!(reader.metric(&free, 10).unwrap().series.is_empty());
}

#[test]
fn sql_reads_and_cannot_do_more() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let result = reader
        .sql(
            "SELECT service, count(*) AS n FROM resources GROUP BY service ORDER BY service",
            1,
        )
        .unwrap();
    assert_eq!(result.columns, ["service", "n"]);
    assert_eq!(result.rows, [vec![json!("api"), json!(2)]]);
    assert!(result.truncated);
    assert!(reader.sql("PRAGMA table_info(logs)", 100).is_ok());

    let today = Day::today();
    for sql in [
        format!("DELETE FROM \"{today}\".logs"),
        "ATTACH DATABASE '/tmp/other.sqlite' AS other".into(),
        "PRAGMA query_only = false".into(),
        "SELECT 1; SELECT 2".into(),
    ] {
        assert!(reader.sql(&sql, 10).is_err(), "{sql} ran");
    }
    // A denied statement leaves the reader working.
    assert!(reader.logs(&LogFilter::default(), 1).is_ok());
}

#[test]
fn a_query_stops_at_the_time_limit() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    reader.set_time_limit(Duration::from_millis(50)).unwrap();
    let error = reader
        .sql(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n) SELECT max(i) FROM n",
            1,
        )
        .unwrap_err();
    assert!(timed_out(&error), "{error:#}");
}
