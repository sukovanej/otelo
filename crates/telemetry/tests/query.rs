use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use serde_json::{Map, Value, json};
use siner_query::{Signal, complete, parse};
use siner_telemetry::query::{
    MetricFilter, ReaderCatalog, default_step, level, parse_severity, parse_trace_id,
};
use siner_telemetry::{
    Batch, Config, Day, IndexedKey, Indexes, Log, Metric, MetricKind, Point, Reader, Records,
    Resource, Span, Writer, channel, timed_out,
};

const SECOND: i64 = 1_000_000_000;
const TRACE: [u8; 16] = [0xab; 16];
const TRACE_HEX: &str = "abababababababababababababababab";

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn log(ts: i64, severity: i32, body: &str, attributes: &Value) -> Log {
    Log {
        ts,
        severity,
        body: body.into(),
        trace_id: None,
        span_id: None,
        attributes: object(attributes),
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

fn records(service: &str, attributes: &Value) -> Records {
    Records {
        resource: Resource {
            service: service.into(),
            attributes: object(attributes),
        },
        logs: Vec::new(),
        spans: Vec::new(),
        metrics: Vec::new(),
    }
}

fn write(dir: &Path, batch: Batch, indexes: &Indexes) {
    let (sender, inbox) = channel(1);
    if !batch.is_empty() {
        sender.send(batch);
    }
    let mut config = Config::new(dir.to_owned());
    config.indexes = indexes.clone();
    let writer = Writer::spawn(config, inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn indexed(keys: &[(Signal, &str)]) -> BTreeSet<IndexedKey> {
    keys.iter()
        .map(|&(signal, key)| IndexedKey::new(signal, key).unwrap())
        .collect()
}

/// A day file for yesterday and one for today. `start` is the first
/// nanosecond of today.
struct Fixture {
    dir: tempfile::TempDir,
    start: i64,
}

impl Fixture {
    fn new() -> Self {
        Self::with_indexes(&Indexes::default())
    }

    fn with_indexes(indexes: &Indexes) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let start = Day::today().start();
        let (y, t) = (start - 60 * SECOND, start + SECOND);

        let mut api = records(
            "api",
            &json!({"service.name": "api", "host.name": "droplet"}),
        );
        api.logs = vec![
            log(
                y,
                9,
                "user 7 signed in",
                &json!({"user.id": 7, "http.route": "/login"}),
            ),
            log(
                t,
                9,
                "user 8 signed in",
                &json!({"user.id": "8", "http.route": "/login"}),
            ),
            log(
                t + SECOND,
                17,
                "payment 12 failed: card \"visa\" declined",
                &json!({"user.id": 7, "http.route": "/matches", "http.response.status_code": 500}),
            ),
            log(
                t + 2 * SECOND,
                17,
                "payment 13 failed: card \"amex\" declined",
                &json!({"user.id": 9, "http.route": "/matches", "http.response.status_code": 200}),
            ),
            Log {
                trace_id: Some(TRACE),
                span_id: Some([2; 8]),
                ..log(t + 3 * SECOND, 13, "slow query GET /languages", &json!({}))
            },
        ];
        let mut root = span(TRACE, 1, None, t, "GET /languages");
        root.duration_ns = 900_000_000;
        root.attributes = object(&json!({"http.route": "/languages"}));
        let mut failed = span(TRACE, 2, Some(1), t + 2 * SECOND, "SELECT languages");
        failed.status = 2;
        failed.kind = 3;
        failed.attributes = object(&json!({"db.system": "sqlite"}));
        let mut matches = span([0xef; 16], 1, None, t + SECOND, "POST /matches");
        matches.attributes = object(&json!({"http.route": "/matches", "user.id": 7}));
        api.spans = vec![
            root,
            failed,
            span([0xcd; 16], 1, None, y, "GET /health"),
            matches,
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
        let mut caddy = records("caddy", &json!({"service.name": "caddy"}));
        caddy.logs = vec![log(
            t,
            9,
            "served 200 in 3ms",
            &json!({"user.id": 7, "http.route": "/languages", "http.response.status_code": 200}),
        )];
        write(dir.path(), vec![api, caddy], indexes);
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

fn logs(reader: &Reader, query: &str) -> Vec<String> {
    let query = parse(query, Signal::Logs).unwrap();
    let logs = reader.logs(&query, 100).unwrap();
    logs.logs.into_iter().map(|line| line.body).collect()
}

fn spans(reader: &Reader, query: &str) -> Vec<String> {
    let query = parse(query, Signal::Spans).unwrap();
    let spans = reader.spans(&query, 100).unwrap();
    spans.spans.into_iter().map(|span| span.name).collect()
}

fn traces(reader: &Reader, query: &str) -> Vec<String> {
    let query = parse(query, Signal::Spans).unwrap();
    let traces = reader.traces(&query, 100).unwrap();
    traces.traces.into_iter().map(|trace| trace.name).collect()
}

#[test]
fn logs_come_newest_first_and_say_when_they_are_cut() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let all = parse("", Signal::Logs).unwrap();
    let logs = reader.logs(&all, 3).unwrap();
    assert!(logs.truncated);
    let bodies: Vec<&str> = logs.logs.iter().map(|l| l.body.as_str()).collect();
    assert_eq!(
        bodies,
        [
            "slow query GET /languages",
            "payment 13 failed: card \"amex\" declined",
            "payment 12 failed: card \"visa\" declined",
        ]
    );
    assert_eq!(logs.logs[0].level, "WARN");
    assert_eq!(logs.logs[0].trace_id.as_deref(), Some(TRACE_HEX));
    assert_eq!(logs.logs[0].resource["host.name"], "droplet");
    assert!(!reader.logs(&all, 7).unwrap().truncated);
}

#[test]
fn a_query_combines_attributes_with_and_or() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    assert_eq!(
        logs(
            &reader,
            r#"http.route = "/matches" OR (user.id = 7 AND http.response.status_code = 200)"#
        ),
        [
            "payment 13 failed: card \"amex\" declined",
            "payment 12 failed: card \"visa\" declined",
            "served 200 in 3ms",
        ]
    );
}

#[test]
fn numbers_match_numbers_and_strings_of_them() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    assert_eq!(logs(&reader, "user.id = 8"), ["user 8 signed in"]);
    assert_eq!(logs(&reader, "user.id = \"7\"").len(), 3);
    assert_eq!(
        logs(&reader, "http.response.status_code >= 500"),
        ["payment 12 failed: card \"visa\" declined"]
    );
    assert_eq!(logs(&reader, "user.id in (8, 9)").len(), 2);
}

#[test]
fn not_and_not_equal_keep_records_without_the_attribute() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let expected = [
        "slow query GET /languages",
        "payment 13 failed: card \"amex\" declined",
        "user 8 signed in",
    ];
    assert_eq!(logs(&reader, "NOT user.id = 7"), expected);
    assert_eq!(logs(&reader, "user.id != 7"), expected);
}

#[test]
fn builtin_fields_filter_logs() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    assert_eq!(logs(&reader, "service = caddy"), ["served 200 in 3ms"]);
    assert_eq!(logs(&reader, "level >= error").len(), 2);
    assert_eq!(logs(&reader, "level = warn"), ["slow query GET /languages"]);
    assert_eq!(
        logs(&reader, &format!("trace_id = {TRACE_HEX}")),
        ["slow query GET /languages"]
    );
    // The words are found in both day files, and punctuation is no syntax.
    assert_eq!(
        logs(&reader, "body ~ \"signed IN\""),
        ["user 8 signed in", "user 7 signed in"]
    );
    assert_eq!(
        logs(&reader, "body ~ \"GET /languages\""),
        ["slow query GET /languages"]
    );
}

#[test]
fn resource_attributes_contains_and_has() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    assert_eq!(logs(&reader, "resource.host.name = droplet").len(), 5);
    assert_eq!(logs(&reader, "has(http.response.status_code)").len(), 3);
    assert_eq!(logs(&reader, "http.route ~ match").len(), 2);
    assert_eq!(
        logs(&reader, "http.route in (/login, /languages)"),
        ["served 200 in 3ms", "user 8 signed in", "user 7 signed in"]
    );
}

#[test]
fn an_invalid_query_says_why() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    for (query, message) in [
        ("level = loud", "\"loud\" is not a severity"),
        ("trace_id = xyz", "is not a trace ID"),
        ("has(service)", "has() takes an attribute"),
        ("level ~ warn", "~ takes a text field"),
    ] {
        let query = parse(query, Signal::Logs).unwrap();
        let error = reader.logs(&query, 10).unwrap_err();
        assert!(
            error
                .downcast_ref::<siner_telemetry::query::InvalidQuery>()
                .is_some(),
            "{error:#}"
        );
        assert!(error.to_string().contains(message), "{error:#}");
    }
}

#[test]
fn log_groups_count_lines_by_template() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let groups = reader
        .log_groups(&parse("service = api", Signal::Logs).unwrap(), 2)
        .unwrap();
    assert!(groups.truncated);
    assert!(!groups.partial);
    assert_eq!(groups.scanned, 5);
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
    assert_eq!(payment.samples.len(), 2);
}

#[test]
fn traces_match_on_any_of_their_spans() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let all = reader
        .traces(&parse("", Signal::Spans).unwrap(), 10)
        .unwrap();
    let found: Vec<(&str, u64, bool)> = all
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
    assert_eq!(traces(&reader, "error = true"), ["GET /languages"]);
    assert_eq!(traces(&reader, "db.system = sqlite"), ["GET /languages"]);
    assert_eq!(
        traces(&reader, "root = true AND duration > 500ms"),
        ["GET /languages"]
    );
    assert_eq!(
        traces(&reader, "name ~ GET"),
        ["GET /languages", "GET /health"]
    );
    assert_eq!(traces(&reader, "user.id = 7"), ["POST /matches"]);
}

#[test]
fn spans_list_the_matching_spans() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    assert_eq!(spans(&reader, "kind = client"), ["SELECT languages"]);
    assert_eq!(spans(&reader, "status = error"), ["SELECT languages"]);
    assert_eq!(spans(&reader, "error = false AND root = true").len(), 3);
    let query = parse("db.system = sqlite", Signal::Spans).unwrap();
    let span = &reader.spans(&query, 10).unwrap().spans[0];
    assert_eq!(span.trace_id, TRACE_HEX);
    assert_eq!(span.resource["service.name"], "api");
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
fn metrics_filter_series_by_labels_and_resource() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let metrics = |query: &str| {
        let list = reader
            .metrics(&parse(query, Signal::Metrics).unwrap(), 10)
            .unwrap();
        list.series.into_iter().map(|s| s.name).collect::<Vec<_>>()
    };
    assert_eq!(metrics("state = used"), ["process.memory.usage"]);
    assert_eq!(
        metrics("name = process.memory.usage resource.host.name = droplet"),
        ["process.memory.usage"]
    );
    assert!(metrics("service = caddy").is_empty());

    let filter = MetricFilter {
        name: "process.memory.usage".into(),
        query: parse("state = used", Signal::Metrics).unwrap(),
        step_ns: 60 * SECOND,
    };
    let metric = reader.metric(&filter, 10).unwrap();
    assert_eq!(metric.series.len(), 1);
    assert_eq!(metric.series[0].resource["host.name"], "droplet");
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
        query: parse("state = free", Signal::Metrics).unwrap(),
        ..filter
    };
    assert!(reader.metric(&free, 10).unwrap().series.is_empty());
}

#[test]
fn the_catalog_knows_the_attributes_and_their_values() {
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let attributes = reader.attributes(Signal::Logs).unwrap();
    let keys: Vec<(&str, &str, u64)> = attributes
        .record
        .iter()
        .map(|a| (a.key.as_str(), a.kind.as_str(), a.count))
        .collect();
    assert_eq!(
        keys,
        [
            ("http.route", "string", 5),
            ("user.id", "mixed", 5),
            ("http.response.status_code", "int", 3),
        ]
    );
    let resource: Vec<&str> = attributes.resource.iter().map(|a| a.key.as_str()).collect();
    assert_eq!(resource, ["service.name", "host.name"]);

    let at = |signal: Signal, input: &str| -> Vec<String> {
        complete(input, input.len(), signal, &ReaderCatalog(&reader))
            .into_iter()
            .map(|s| s.text)
            .collect()
    };
    assert_eq!(
        at(Signal::Logs, "http.route = "),
        [r#""/login""#, r#""/matches""#, r#""/languages""#]
    );
    assert_eq!(
        at(Signal::Logs, "http.r"),
        ["http.route", "http.response.status_code"]
    );
    assert_eq!(
        at(Signal::Logs, "resource."),
        ["resource.service.name", "resource.host.name"]
    );
    assert_eq!(at(Signal::Logs, "service = c"), [r#""caddy""#]);
    assert_eq!(at(Signal::Spans, "name = \"SEL"), [r#""SELECT languages""#]);
    assert_eq!(at(Signal::Spans, "db."), ["db.system"]);
    assert_eq!(at(Signal::Metrics, "st"), ["state"]);
    assert_eq!(
        at(Signal::Metrics, "name = p"),
        [r#""process.memory.usage""#]
    );
}

#[test]
fn an_indexed_attribute_has_an_index_in_every_day_file() {
    let indexes = Indexes::new(indexed(&[(Signal::Logs, "user.id")]));
    let fixture = Fixture::with_indexes(&indexes);
    let names = |day: Day| -> Vec<String> {
        let conn = Connection::open(fixture.dir.path().join(day.file_name())).unwrap();
        conn.prepare("SELECT name FROM sqlite_master WHERE name GLOB 'attr_*'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let today = Day::today();
    assert_eq!(names(today).len(), 1);
    assert_eq!(names(today.plus(-1)), names(today));

    // The query the reader runs reads the index, also for OR and NOT.
    let reader = fixture.reader();
    let index = &names(today)[0];
    for query in ["user.id = 7", "user.id = 7 OR user.id in (8, 9)"] {
        let plan = reader
            .explain(&parse(query, Signal::Logs).unwrap())
            .unwrap();
        assert!(
            plan.iter().any(|step| step.contains(index)),
            "{query}: {plan:?}"
        );
    }
    let plan = reader
        .explain(&parse("http.route = x", Signal::Logs).unwrap())
        .unwrap();
    assert!(!plan.iter().any(|step| step.contains("attr_")), "{plan:?}");
    assert!(
        reader
            .explain(&parse("", Signal::Metrics).unwrap())
            .is_err()
    );

    let mut reader = fixture.reader();
    let query = parse("user.id = 7 OR http.route = x", Signal::Logs).unwrap();
    assert_eq!(
        reader.logs(&query, 10).unwrap().unindexed,
        ["user.id", "http.route"]
    );
    reader.set_indexes(indexes.get());
    assert_eq!(reader.logs(&query, 10).unwrap().unindexed, ["http.route"]);

    // A later writer drops the index when the set loses the key.
    indexes.set(BTreeSet::new());
    write(fixture.dir.path(), Vec::new(), &indexes);
    assert!(names(today).is_empty());
    assert!(names(today.plus(-1)).is_empty());
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
    assert!(reader.sql("SELECT * FROM attribute_keys", 100).is_ok());

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
    assert!(reader.logs(&parse("", Signal::Logs).unwrap(), 1).is_ok());
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

#[test]
fn a_day_file_from_before_the_catalog_gets_its_tables() {
    let dir = tempfile::tempdir().unwrap();
    let yesterday = Day::today().plus(-1);
    let conn = Connection::open(dir.path().join(yesterday.file_name())).unwrap();
    conn.execute_batch(
        "CREATE TABLE logs (ts INTEGER NOT NULL, resource_id INTEGER NOT NULL,
           severity INTEGER NOT NULL, body TEXT NOT NULL, trace_id BLOB, span_id BLOB,
           attributes TEXT NOT NULL, source TEXT NOT NULL)",
    )
    .unwrap();
    drop(conn);
    write(dir.path(), Vec::new(), &Indexes::default());
    let reader = Reader::open(dir.path(), yesterday.start(), Day::today().start()).unwrap();
    assert!(reader.attributes(Signal::Logs).unwrap().record.is_empty());
}

#[test]
fn a_histogram_returns_the_bucket_counts_of_each_step() {
    let dir = tempfile::tempdir().unwrap();
    let start = Day::today().start();
    let histogram = |counts: [u64; 3], sum: f64| siner_telemetry::Histogram {
        bounds: vec![0.1, 1.0],
        counts: counts.to_vec(),
        count: counts.iter().sum(),
        sum: Some(sum),
        min: None,
        max: None,
        cumulative: true,
    };
    let point = |offset: i64, histogram| Point {
        ts: start + offset * SECOND,
        value: 0.0,
        histogram: Some(histogram),
    };
    let mut broken = histogram([1, 1, 1], 1.0);
    broken.counts.pop();
    let mut api = records("api", &json!({"service.name": "api"}));
    api.metrics = vec![Metric {
        name: "http.server.request.duration".into(),
        kind: MetricKind::Histogram,
        unit: "s".into(),
        labels: object(&json!({"http.route": "/matches"})),
        points: vec![
            point(1, histogram([10, 2, 0], 3.0)),
            point(30, histogram([14, 5, 1], 7.5)),
            // Counts that do not fit the bounds are not stored.
            point(40, broken),
            point(70, histogram([20, 5, 1], 8.0)),
        ],
    }];
    write(dir.path(), vec![api], &Indexes::default());
    let reader = Reader::open(dir.path(), start, start + 600 * SECOND).unwrap();
    let filter = MetricFilter {
        name: "http.server.request.duration".into(),
        query: parse("", Signal::Metrics).unwrap(),
        step_ns: 60 * SECOND,
    };
    let metric = reader.metric(&filter, 10).unwrap();
    let buckets = &metric.series[0].buckets;
    assert_eq!(buckets[0].count, 2);
    let first = buckets[0].histogram.as_ref().unwrap();
    assert_eq!(first.bounds, [0.1, 1.0]);
    assert_eq!(first.counts, [4, 3, 1]);
    assert_eq!(first.count, 8);
    assert_eq!(first.sum, Some(4.5));
    assert!(first.p50.unwrap() > 0.0 && first.p50.unwrap() <= 0.1);
    assert_eq!(first.p99, Some(1.0));
    let second = buckets[1].histogram.as_ref().unwrap();
    assert_eq!(second.counts, [6, 0, 0]);
}

#[test]
fn parses_trace_ids_and_severities() {
    let id = parse_trace_id("4bf92f3577b34da6a3ce929d0e0e4736").unwrap();
    assert_eq!(id[..3], [0x4b, 0xf9, 0x2f]);
    assert_eq!(id[15], 0x36);
    assert!(parse_trace_id("4bf9").is_err());
    assert!(parse_trace_id("zzf92f3577b34da6a3ce929d0e0e4736").is_err());
    assert_eq!(parse_severity("WARN").unwrap(), 13);
    assert_eq!(parse_severity("17").unwrap(), 17);
    assert!(parse_severity("loud").is_err());
    assert_eq!(level(18), "ERROR");
}

#[test]
fn picks_a_step_that_fits_the_range() {
    assert_eq!(default_step(0, 3600 * SECOND), 30 * SECOND);
    assert_eq!(default_step(0, 60 * SECOND), SECOND);
    assert_eq!(default_step(0, 7 * 86_400 * SECOND), 3 * 3600 * SECOND);
    assert_eq!(default_step(0, 1000 * 86_400 * SECOND), 86_400 * SECOND);
}

#[test]
fn the_catalog_stops_keeping_values_of_a_key_with_many() {
    let dir = tempfile::tempdir().unwrap();
    let t = Day::today().start() + SECOND;
    let mut api = records("api", &json!({"service.name": "api"}));
    // 201 numbers, a repeat, and a string: more values than a key keeps.
    api.logs = (0..=200)
        .map(|id| log(t, 9, "signed in", &json!({ "user.id": id })))
        .chain([
            log(t, 9, "signed in", &json!({"user.id": 3})),
            log(t, 9, "signed in", &json!({"user.id": "x"})),
        ])
        .collect();
    write(dir.path(), vec![api], &Indexes::default());
    let reader = Reader::open(dir.path(), t, t + SECOND).unwrap();

    let attributes = reader.attributes(Signal::Logs).unwrap();
    let user = &attributes.record[0];
    assert_eq!(
        (user.key.as_str(), user.kind.as_str(), user.count),
        ("user.id", "mixed", 203)
    );
    let row = |sql: &str| reader.sql(sql, 10).unwrap().rows;
    assert_eq!(
        row("SELECT many_values FROM attribute_keys WHERE key = 'user.id'"),
        [vec![json!(1)]]
    );
    assert_eq!(
        row("SELECT count(*) FROM attribute_values WHERE key = 'user.id'"),
        [vec![json!(200)]]
    );
    assert_eq!(
        row("SELECT count FROM attribute_values WHERE key = 'user.id' AND value = '3'"),
        [vec![json!(2)]]
    );
}
