use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;

use otelo_indexed_storage::query::{
    Bucket, BucketChange, GroupKey, Grouping, MetricFilter, Resolution, SeriesGroup,
};
use otelo_indexed_storage::{
    AttributeValue, Attributes, Batch, Buckets, Distribution, Error, ExplicitBuckets,
    ExponentialBuckets, Histogram, HistogramPoint, IndexedAttribute, IndexedCounts, IndexedSignal,
    Log, LogSource, Metric, MetricKind, NumberPoint, Points, RangeQueries, Records, Resource,
    Severity, Span, SpanId, SpanKind, SpanStatus, Temporality, TimeRange, TraceContext, TraceId,
    open_batch_channel,
};
use otelo_indexed_storage_sqlite::{Config, Day, Indexes, Reader, Writer};
use otelo_query::{FieldOrigin, MAX_HELP_VALUES, Signal, ValueType, complete_query, parse_query};
use rusqlite::Connection;
use serde_json::{Value, json};

fn attributes_from_json(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

const SECOND: i64 = 1_000_000_000;
const TRACE_ID: TraceId = TraceId([0xab; 16]);
const TRACE_ID_HEX: &str = "abababababababababababababababab";

fn log(logged_at: i64, severity: Severity, body: &str, attributes: &Value) -> Log {
    Log {
        logged_at,
        severity,
        body: body.into(),
        trace_context: TraceContext::None,
        attributes: attributes_from_json(attributes.clone()),
        source: LogSource::Otlp,
    }
}

fn span(
    trace_id: TraceId,
    span_id_byte: u8,
    parent_span_id_byte: Option<u8>,
    started_at: i64,
    name: &str,
) -> Span {
    Span {
        trace_id,
        span_id: SpanId([span_id_byte; 8]),
        parent_span_id: parent_span_id_byte.map(|byte| SpanId([byte; 8])),
        name: name.into(),
        kind: SpanKind::Server,
        started_at,
        duration_ns: 10_000_000,
        status: SpanStatus::Unset,
        attributes: Attributes::new(),
        events: Vec::new(),
    }
}

fn memory_usage_metric(points: &[(i64, f64)]) -> Metric {
    Metric {
        name: "process.memory.usage".into(),
        unit: "By".into(),
        labels: attributes_from_json(json!({"state": "used"})),
        points: Points::UpDown(
            points
                .iter()
                .map(|&(recorded_at, value)| NumberPoint { recorded_at, value })
                .collect(),
        ),
    }
}

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

fn distribution_of(bucket: &Bucket) -> &Distribution {
    match &bucket.change {
        BucketChange::Distribution(distribution) => distribution,
        other => panic!("a distribution, not {other:?}"),
    }
}

fn labels_and_resource_of(group: &SeriesGroup) -> (&Attributes, &Attributes) {
    match &group.key {
        GroupKey::Series {
            labels, resource, ..
        } => (labels, resource),
        other => panic!("one series, not {other:?}"),
    }
}

const fn rate_of(bucket: &Bucket) -> Option<f64> {
    match bucket.change {
        BucketChange::Rate { per_second } => Some(per_second),
        BucketChange::None | BucketChange::Distribution(_) => None,
    }
}

fn write_batch(directory: &Path, batch: Batch, indexes: &Indexes) {
    let (sender, inbox) = open_batch_channel(1);
    if !batch.is_empty() {
        assert!(sender.send_batch(batch));
    }
    let mut config = Config::new(directory.to_owned());
    config.indexes = indexes.clone();
    let writer = Writer::spawn(config, inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn indexed_attributes(keys: &[(IndexedSignal, &str)]) -> BTreeSet<IndexedAttribute> {
    keys.iter()
        .map(|&(signal, key)| IndexedAttribute::new(signal, key).unwrap())
        .collect()
}

struct Fixture {
    directory: tempfile::TempDir,
    today_start_at: i64,
}

impl Fixture {
    fn new() -> Self {
        Self::with_indexes(&Indexes::default())
    }

    fn with_indexes(indexes: &Indexes) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let today_start_at = Day::today().start_at();
        let (yesterday_at, today_at) = (today_start_at - 60 * SECOND, today_start_at + SECOND);

        let mut api = records(
            "api",
            &json!({"service.name": "api", "host.name": "droplet"}),
        );
        api.logs = vec![
            log(
                yesterday_at,
                Severity::INFO,
                "user 7 signed in",
                &json!({"user.id": 7, "http.route": "/login"}),
            ),
            log(
                today_at,
                Severity::INFO,
                "user 8 signed in",
                &json!({"user.id": "8", "http.route": "/login"}),
            ),
            log(
                today_at + SECOND,
                Severity::ERROR,
                "payment 12 failed: card \"visa\" declined",
                &json!({"user.id": 7, "http.route": "/matches", "http.response.status_code": 500}),
            ),
            log(
                today_at + 2 * SECOND,
                Severity::ERROR,
                "payment 13 failed: card \"amex\" declined",
                &json!({"user.id": 9, "http.route": "/matches", "http.response.status_code": 200}),
            ),
            Log {
                trace_context: TraceContext::Span {
                    trace_id: TRACE_ID,
                    span_id: SpanId([2; 8]),
                },
                ..log(
                    today_at + 3 * SECOND,
                    Severity::WARN,
                    "slow query GET /languages",
                    &json!({}),
                )
            },
        ];
        let mut root = span(TRACE_ID, 1, None, today_at, "GET /languages");
        root.duration_ns = 900_000_000;
        root.attributes = attributes_from_json(json!({"http.route": "/languages"}));
        let mut failed = span(
            TRACE_ID,
            2,
            Some(1),
            today_at + 2 * SECOND,
            "SELECT languages",
        );
        failed.status = SpanStatus::Error;
        failed.kind = SpanKind::Client;
        failed.attributes = attributes_from_json(json!({"db.system": "sqlite"}));
        let mut matches = span(
            TraceId([0xef; 16]),
            1,
            None,
            today_at + SECOND,
            "POST /matches",
        );
        matches.attributes = attributes_from_json(json!({"http.route": "/matches", "user.id": 7}));
        api.spans = vec![
            root,
            failed,
            span(TraceId([0xcd; 16]), 1, None, yesterday_at, "GET /health"),
            matches,
        ];
        api.metrics = vec![memory_usage_metric(&[
            (today_at, 100.0),
            (today_at + 10 * SECOND, 300.0),
            (today_at + 70 * SECOND, 50.0),
        ])];
        let mut caddy = records("caddy", &json!({"service.name": "caddy"}));
        caddy.logs = vec![log(
            today_at,
            Severity::INFO,
            "served 200 in 3ms",
            &json!({"user.id": 7, "http.route": "/languages", "http.response.status_code": 200}),
        )];
        write_batch(directory.path(), vec![api, caddy], indexes);
        Self {
            directory,
            today_start_at,
        }
    }

    fn reader_around_midnight(&self) -> Reader {
        Reader::open(
            self.directory.path(),
            TimeRange::new(
                self.today_start_at - 3600 * SECOND,
                self.today_start_at + 600 * SECOND,
            )
            .unwrap(),
        )
        .unwrap()
    }
}

fn log_bodies_matching(reader: &Reader, query: &str) -> Vec<String> {
    let query = parse_query(query, Signal::Logs).unwrap();
    let logs = reader.list_logs(&query, 100).unwrap();
    logs.logs.into_iter().map(|line| line.body).collect()
}

fn span_names_matching(reader: &Reader, query: &str) -> Vec<String> {
    let query = parse_query(query, Signal::Spans).unwrap();
    let spans = reader.list_spans(&query, 100).unwrap();
    spans.spans.into_iter().map(|span| span.name).collect()
}

fn trace_names_matching(reader: &Reader, query: &str) -> Vec<String> {
    let query = parse_query(query, Signal::Spans).unwrap();
    let traces = reader.list_traces(&query, 100).unwrap();
    traces.traces.into_iter().map(|trace| trace.name).collect()
}

#[test]
fn logs_come_newest_first_and_say_when_they_are_cut() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    let every_log = parse_query("", Signal::Logs).unwrap();
    let logs = reader.list_logs(&every_log, 3).unwrap();
    assert!(logs.truncated);
    let bodies: Vec<&str> = logs.logs.iter().map(|line| line.body.as_str()).collect();
    assert_eq!(
        bodies,
        [
            "slow query GET /languages",
            "payment 13 failed: card \"amex\" declined",
            "payment 12 failed: card \"visa\" declined",
        ]
    );
    assert_eq!(logs.logs[0].severity.level(), "WARN");
    assert_eq!(logs.logs[0].trace_id, Some(TRACE_ID));
    assert_eq!(logs.logs[0].resource["host.name"], "droplet");
    assert!(!reader.list_logs(&every_log, 7).unwrap().truncated);
}

#[test]
fn a_query_combines_attributes_with_and_or() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    assert_eq!(
        log_bodies_matching(
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
    let reader = fixture.reader_around_midnight();
    assert_eq!(
        log_bodies_matching(&reader, "user.id = 8"),
        ["user 8 signed in"]
    );
    assert_eq!(log_bodies_matching(&reader, "user.id = \"7\"").len(), 3);
    assert_eq!(
        log_bodies_matching(&reader, "http.response.status_code >= 500"),
        ["payment 12 failed: card \"visa\" declined"]
    );
    assert_eq!(log_bodies_matching(&reader, "user.id in (8, 9)").len(), 2);
}

#[test]
fn not_and_not_equal_keep_records_without_the_attribute() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    let expected = [
        "slow query GET /languages",
        "payment 13 failed: card \"amex\" declined",
        "user 8 signed in",
    ];
    assert_eq!(log_bodies_matching(&reader, "NOT user.id = 7"), expected);
    assert_eq!(log_bodies_matching(&reader, "user.id != 7"), expected);
}

#[test]
fn builtin_fields_filter_logs() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    assert_eq!(
        log_bodies_matching(&reader, "service = caddy"),
        ["served 200 in 3ms"]
    );
    assert_eq!(log_bodies_matching(&reader, "level >= error").len(), 2);
    assert_eq!(
        log_bodies_matching(&reader, "level = warn"),
        ["slow query GET /languages"]
    );
    assert_eq!(
        log_bodies_matching(&reader, &format!("trace_id = {TRACE_ID_HEX}")),
        ["slow query GET /languages"]
    );
    assert_eq!(
        log_bodies_matching(&reader, "body ~ \"signed IN\""),
        ["user 8 signed in", "user 7 signed in"]
    );
    assert_eq!(
        log_bodies_matching(&reader, "body ~ \"GET /languages\""),
        ["slow query GET /languages"]
    );
}

#[test]
fn resource_attributes_contains_and_has() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    assert_eq!(
        log_bodies_matching(&reader, "resource.host.name = droplet").len(),
        5
    );
    assert_eq!(
        log_bodies_matching(&reader, "has(http.response.status_code)").len(),
        3
    );
    assert_eq!(log_bodies_matching(&reader, "http.route ~ match").len(), 2);
    assert_eq!(
        log_bodies_matching(&reader, "http.route in (/login, /languages)"),
        ["served 200 in 3ms", "user 8 signed in", "user 7 signed in"]
    );
}

#[test]
fn an_invalid_query_says_why() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    for (query, message) in [
        ("level = loud", "\"loud\" is not a severity"),
        ("trace_id = xyz", "is not a trace ID"),
        ("has(service)", "has() takes an attribute"),
        ("level ~ warn", "~ takes a text field"),
    ] {
        let query = parse_query(query, Signal::Logs).unwrap();
        let error = reader.list_logs(&query, 10).unwrap_err();
        assert!(matches!(error, Error::InvalidQuery(_)), "{error:#}");
        assert!(error.to_string().contains(message), "{error:#}");
    }
}

#[test]
fn log_groups_count_lines_by_template() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    let groups = reader
        .list_log_groups(&parse_query("service = api", Signal::Logs).unwrap(), 2)
        .unwrap();
    assert!(groups.truncated);
    assert!(!groups.partial);
    assert_eq!(groups.scanned, 5);
    let found: Vec<(&str, u64)> = groups
        .groups
        .iter()
        .map(|group| (group.template.as_str(), group.count))
        .collect();
    assert_eq!(
        found,
        [
            ("payment <num> failed: card \"<str>\" declined", 2),
            ("user <num> signed in", 2),
        ]
    );
    let payment = &groups.groups[0];
    assert_eq!(payment.severity.level(), "ERROR");
    assert_eq!(payment.services, ["api"]);
    assert_eq!(payment.samples.len(), 2);
}

#[test]
fn traces_match_on_any_of_their_spans() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    let all_traces = reader
        .list_traces(&parse_query("", Signal::Spans).unwrap(), 10)
        .unwrap();
    let found: Vec<(&str, u64, bool)> = all_traces
        .traces
        .iter()
        .map(|trace| (trace.name.as_str(), trace.spans, trace.error))
        .collect();
    assert_eq!(
        found,
        [
            ("POST /matches", 1, false),
            ("GET /languages", 2, true),
            ("GET /health", 1, false),
        ]
    );
    assert_eq!(all_traces.traces[0].attributes["http.route"], "/matches");
    assert_eq!(
        all_traces.traces[0].attributes["user.id"],
        AttributeValue::Int(7)
    );
    assert_eq!(all_traces.traces[0].resource["service.name"], "api");
    assert_eq!(all_traces.traces[0].kind, SpanKind::Server);
    assert_eq!(
        trace_names_matching(&reader, "error = true"),
        ["GET /languages"]
    );
    assert_eq!(
        trace_names_matching(&reader, "db.system = sqlite"),
        ["GET /languages"]
    );
    assert_eq!(
        trace_names_matching(&reader, "root = true AND duration > 500ms"),
        ["GET /languages"]
    );
    assert_eq!(
        trace_names_matching(&reader, "name ~ GET"),
        ["GET /languages", "GET /health"]
    );
    assert_eq!(
        trace_names_matching(&reader, "user.id = 7"),
        ["POST /matches"]
    );
}

#[test]
fn spans_list_the_matching_spans() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    assert_eq!(
        span_names_matching(&reader, "kind = client"),
        ["SELECT languages"]
    );
    assert_eq!(
        span_names_matching(&reader, "status = error"),
        ["SELECT languages"]
    );
    assert_eq!(
        span_names_matching(&reader, "error = false AND root = true").len(),
        3
    );
    let query = parse_query("db.system = sqlite", Signal::Spans).unwrap();
    let span = &reader.list_spans(&query, 10).unwrap().spans[0];
    assert_eq!(span.trace_id, TRACE_ID);
    assert_eq!(span.resource["service.name"], "api");
}

#[test]
fn a_trace_has_its_spans_and_logs() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    let trace = reader.get_trace(TRACE_ID, 10).unwrap().unwrap();
    assert_eq!(trace.trace_id, TRACE_ID);
    let spans: Vec<(&str, Option<SpanId>, bool)> = trace
        .spans
        .iter()
        .map(|span| {
            (
                span.name.as_str(),
                span.parent_span_id,
                span.status.is_error(),
            )
        })
        .collect();
    assert_eq!(
        spans,
        [
            ("GET /languages", None, false),
            ("SELECT languages", Some(SpanId([1; 8])), true)
        ]
    );
    assert_eq!(trace.logs.len(), 1);
    assert!(reader.get_trace(TraceId([0x99; 16]), 10).unwrap().is_none());
}

#[test]
fn metrics_filter_series_by_labels_and_resource() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    let metric_names_matching = |query: &str| {
        let list = reader
            .list_metrics(
                &parse_query(query, Signal::Metrics).unwrap(),
                Resolution::Raw,
                10,
            )
            .unwrap();
        list.series
            .into_iter()
            .map(|series| series.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        metric_names_matching("state = used"),
        ["process.memory.usage"]
    );
    assert_eq!(
        metric_names_matching("name = process.memory.usage resource.host.name = droplet"),
        ["process.memory.usage"]
    );
    assert!(metric_names_matching("service = caddy").is_empty());

    let filter = MetricFilter {
        name: "process.memory.usage".into(),
        query: parse_query("state = used", Signal::Metrics).unwrap(),
        step_ns: 60 * SECOND,
        resolution: Resolution::Raw,
        grouping: Grouping::default(),
    };
    let metric = reader.get_metric_series(&filter, 10).unwrap();
    assert_eq!(metric.groups.len(), 1);
    assert_eq!(metric.groups[0].kind, MetricKind::UpDown);
    assert_eq!(
        labels_and_resource_of(&metric.groups[0]).1["host.name"],
        "droplet"
    );
    let buckets: Vec<(u64, f64, f64, f64, f64)> = metric.groups[0]
        .buckets
        .iter()
        .map(|bucket| {
            (
                bucket.count,
                bucket.min,
                bucket.max,
                bucket.avg,
                bucket.last,
            )
        })
        .collect();
    assert_eq!(
        buckets,
        [(2, 100.0, 300.0, 200.0, 300.0), (1, 50.0, 50.0, 50.0, 50.0)]
    );
    let free = MetricFilter {
        query: parse_query("state = free", Signal::Metrics).unwrap(),
        ..filter
    };
    assert!(
        reader
            .get_metric_series(&free, 10)
            .unwrap()
            .groups
            .is_empty()
    );
}

#[test]
fn the_catalog_knows_the_attributes_and_their_values() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    let attributes = reader.list_attribute_keys(Signal::Logs).unwrap();
    let keys: Vec<(&str, &str, u64)> = attributes
        .record
        .iter()
        .map(|attribute| {
            (
                attribute.key.as_str(),
                attribute.value_type.name(),
                attribute.count,
            )
        })
        .collect();
    assert_eq!(
        keys,
        [
            ("http.route", "string", 5),
            ("user.id", "mixed", 5),
            ("http.response.status_code", "int", 3),
        ]
    );
    let resource: Vec<&str> = attributes
        .resource
        .iter()
        .map(|attribute| attribute.key.as_str())
        .collect();
    assert_eq!(resource, ["service.name", "host.name"]);

    let suggestions_at_end = |signal: Signal, input: &str| -> Vec<String> {
        complete_query(input, input.len(), signal, &reader)
            .suggestions
            .into_iter()
            .map(|suggestion| suggestion.text)
            .collect()
    };
    assert_eq!(
        suggestions_at_end(Signal::Logs, "http.route = "),
        [r#""/login""#, r#""/matches""#, r#""/languages""#]
    );
    assert_eq!(
        suggestions_at_end(Signal::Logs, "http.r"),
        ["http.route", "http.response.status_code"]
    );
    assert_eq!(
        suggestions_at_end(Signal::Logs, "resource."),
        ["resource.service.name", "resource.host.name"]
    );
    assert_eq!(
        suggestions_at_end(Signal::Logs, "service = c"),
        [r#""caddy""#]
    );
    assert_eq!(
        suggestions_at_end(Signal::Spans, "name = \"SEL"),
        [r#""SELECT languages""#]
    );
    assert_eq!(suggestions_at_end(Signal::Spans, "db."), ["db.system"]);
    assert_eq!(suggestions_at_end(Signal::Metrics, "st"), ["state"]);
    assert_eq!(
        suggestions_at_end(Signal::Metrics, "name = p"),
        [r#""process.memory.usage""#]
    );

    let route = complete_query("http.route", 10, Signal::Logs, &reader)
        .help_for_field_at_cursor
        .unwrap();
    assert_eq!(route.value_type, ValueType::String);
    assert_eq!(route.origin, FieldOrigin::Attribute { record_count: 5 });
    let values: Vec<&str> = route
        .most_common_values
        .iter()
        .map(|value| value.text.as_str())
        .collect();
    assert_eq!(values, [r#""/login""#, r#""/matches""#, r#""/languages""#]);
    assert_eq!(route.distinct_value_count, 3);
    assert!(!route.has_more_values_than_listed);
}

#[test]
fn an_indexed_attribute_has_an_index_in_every_day_file() {
    let indexes = Indexes::new(indexed_attributes(&[(IndexedSignal::Logs, "user.id")]));
    let fixture = Fixture::with_indexes(&indexes);
    let index_names = |day: Day| -> Vec<String> {
        let connection = Connection::open(fixture.directory.path().join(day.file_name())).unwrap();
        connection
            .prepare("SELECT name FROM sqlite_master WHERE name GLOB 'logs_attribute_*'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let today = Day::today();
    assert_eq!(index_names(today).len(), 1);
    assert_eq!(index_names(today.add_days(-1)), index_names(today));

    let reader = fixture.reader_around_midnight();
    let index = &index_names(today)[0];
    for query in ["user.id = 7", "user.id = 7 OR user.id in (8, 9)"] {
        let plan = reader
            .explain_query(&parse_query(query, Signal::Logs).unwrap())
            .unwrap();
        assert!(
            plan.iter().any(|step| step.contains(index)),
            "{query}: {plan:?}"
        );
    }
    let plan = reader
        .explain_query(&parse_query("http.route = x", Signal::Logs).unwrap())
        .unwrap();
    assert!(
        !plan.iter().any(|step| step.contains("_attribute_")),
        "{plan:?}"
    );
    assert!(
        reader
            .explain_query(&parse_query("", Signal::Metrics).unwrap())
            .is_err()
    );

    let mut reader = fixture.reader_around_midnight();
    let query = parse_query("user.id = 7 OR http.route = x", Signal::Logs).unwrap();
    assert_eq!(
        reader.list_logs(&query, 10).unwrap().unindexed,
        ["user.id", "http.route"]
    );
    reader.set_indexed_attributes(indexes.attributes());
    assert_eq!(
        reader.list_logs(&query, 10).unwrap().unindexed,
        ["http.route"]
    );

    indexes.replace_attributes(BTreeSet::new());
    write_batch(fixture.directory.path(), Vec::new(), &indexes);
    assert!(index_names(today).is_empty());
    assert!(index_names(today.add_days(-1)).is_empty());
}

#[test]
fn a_query_stops_at_the_time_limit() {
    let fixture = Fixture::new();
    let reader = fixture.reader_around_midnight();
    reader.set_time_limit(Duration::from_millis(50)).unwrap();
    let error = reader
        .connection()
        .query_row(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n) SELECT max(i) FROM n",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_err();
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::OperationInterrupted),
        "{error:#}"
    );
}

#[test]
fn a_histogram_returns_the_bucket_counts_of_each_step() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    let histogram = |counts: [u64; 3], sum: f64| Histogram {
        count: counts.iter().sum(),
        sum: Some(sum),
        min: None,
        max: None,
        buckets: Buckets::Explicit(ExplicitBuckets::new(vec![0.1, 1.0], counts.to_vec()).unwrap()),
    };
    let point = |offset: i64, histogram| HistogramPoint {
        recorded_at: today_start_at + offset * SECOND,
        histogram,
    };
    let mut api = records("api", &json!({"service.name": "api"}));
    api.metrics = vec![Metric {
        name: "http.server.request.duration".into(),
        unit: "s".into(),
        labels: attributes_from_json(json!({"http.route": "/matches"})),
        points: Points::Histogram(
            Temporality::Cumulative,
            vec![
                point(1, histogram([10, 2, 0], 3.0)),
                point(30, histogram([14, 5, 1], 7.5)),
                point(70, histogram([20, 5, 1], 8.0)),
            ],
        ),
    }];
    write_batch(directory.path(), vec![api], &Indexes::default());
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(today_start_at, today_start_at + 600 * SECOND).unwrap(),
    )
    .unwrap();
    let filter = MetricFilter {
        name: "http.server.request.duration".into(),
        query: parse_query("", Signal::Metrics).unwrap(),
        step_ns: 60 * SECOND,
        resolution: Resolution::Raw,
        grouping: Grouping::default(),
    };
    let metric = reader.get_metric_series(&filter, 10).unwrap();
    let buckets = &metric.groups[0].buckets;
    assert_eq!(buckets[0].count, 2);
    let first = distribution_of(&buckets[0]);
    assert_eq!(first.bounds, [0.1, 1.0]);
    assert_eq!(first.counts, [4, 3, 1]);
    assert_eq!(first.count, 8);
    assert_eq!(first.sum, Some(4.5));
    let p50 = first.percentiles.unwrap().p50;
    assert!(p50 > 0.0 && p50 <= 0.1);
    assert_eq!(
        first.percentiles.map(|percentiles| percentiles.p99),
        Some(1.0)
    );
    let second = distribution_of(&buckets[1]);
    assert_eq!(second.counts, [6, 0, 0]);
}

#[test]
fn the_catalog_stops_keeping_values_of_a_key_with_many() {
    let directory = tempfile::tempdir().unwrap();
    let logged_at = Day::today().start_at() + SECOND;
    let mut api = records("api", &json!({"service.name": "api"}));
    api.logs = (0..=200)
        .map(|id| {
            log(
                logged_at,
                Severity::INFO,
                "signed in",
                &json!({ "user.id": id }),
            )
        })
        .chain([
            log(
                logged_at,
                Severity::INFO,
                "signed in",
                &json!({"user.id": 3}),
            ),
            log(
                logged_at,
                Severity::INFO,
                "signed in",
                &json!({"user.id": "x"}),
            ),
        ])
        .collect();
    write_batch(directory.path(), vec![api], &Indexes::default());
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(logged_at, logged_at + SECOND).unwrap(),
    )
    .unwrap();

    let attributes = reader.list_attribute_keys(Signal::Logs).unwrap();
    let user = &attributes.record[0];
    assert_eq!(
        (user.key.as_str(), user.value_type.name(), user.count),
        ("user.id", "mixed", 203)
    );
    let query_integer = |sql: &str| -> i64 {
        reader
            .connection()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    };
    assert_eq!(
        query_integer(
            "SELECT has_more_values_than_listed FROM attribute_keys WHERE key = 'user.id'"
        ),
        1
    );
    assert_eq!(
        query_integer("SELECT count(*) FROM attribute_values WHERE key = 'user.id'"),
        200
    );
    assert_eq!(
        query_integer("SELECT count FROM attribute_values WHERE key = 'user.id' AND value = '3'"),
        2
    );
    let user = complete_query("user.id", 7, Signal::Logs, &reader)
        .help_for_field_at_cursor
        .unwrap();
    assert_eq!(user.distinct_value_count, 200);
    assert_eq!(user.most_common_values.len(), MAX_HELP_VALUES);
    assert!(user.has_more_values_than_listed);
}

#[test]
fn the_catalog_marks_a_key_with_a_value_too_long_to_list() {
    let directory = tempfile::tempdir().unwrap();
    let logged_at = Day::today().start_at() + SECOND;
    let mut api = records("api", &json!({"service.name": "api"}));
    api.logs = vec![
        log(
            logged_at,
            Severity::INFO,
            "asked",
            &json!({"question": "short"}),
        ),
        log(
            logged_at,
            Severity::INFO,
            "asked",
            &json!({"question": "long ".repeat(30)}),
        ),
    ];
    write_batch(directory.path(), vec![api], &Indexes::default());
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(logged_at, logged_at + SECOND).unwrap(),
    )
    .unwrap();

    let question = complete_query("question", 8, Signal::Logs, &reader)
        .help_for_field_at_cursor
        .unwrap();
    assert_eq!(question.distinct_value_count, 1);
    assert!(question.has_more_values_than_listed);
}

#[test]
fn a_counter_returns_its_rate_from_the_point_before_the_range() {
    let directory = tempfile::tempdir().unwrap();
    let range_start_at = Day::today().start_at() + 3600 * SECOND;
    let network_io_metric = |labels: Value, points: &[(i64, f64)]| Metric {
        name: "system.network.io".into(),
        unit: "By".into(),
        labels: attributes_from_json(labels),
        points: Points::Counter(
            Temporality::Cumulative,
            points
                .iter()
                .map(|&(offset, value)| NumberPoint {
                    recorded_at: range_start_at + offset * SECOND,
                    value,
                })
                .collect(),
        ),
    };
    let mut api = records("api", &json!({"service.name": "api"}));
    api.metrics = vec![
        network_io_metric(
            json!({"network.interface.name": "eth0"}),
            // The last value fell: the machine started again.
            &[(-30, 1000.0), (30, 1600.0), (90, 1900.0), (150, 100.0)],
        ),
        network_io_metric(json!({"network.interface.name": "gone"}), &[(-30, 5.0)]),
    ];
    write_batch(directory.path(), vec![api], &Indexes::default());
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(range_start_at, range_start_at + 600 * SECOND).unwrap(),
    )
    .unwrap();
    let filter = MetricFilter {
        name: "system.network.io".into(),
        query: parse_query("", Signal::Metrics).unwrap(),
        step_ns: 60 * SECOND,
        resolution: Resolution::Raw,
        grouping: Grouping::default(),
    };
    let metric = reader.get_metric_series(&filter, 10).unwrap();
    assert_eq!(metric.groups.len(), 1);
    assert!(!metric.truncated);
    let series = &metric.groups[0];
    assert_eq!(series.kind, MetricKind::Counter(Temporality::Cumulative));
    let rates: Vec<(u64, f64, Option<f64>)> = series
        .buckets
        .iter()
        .map(|bucket| (bucket.count, bucket.last, rate_of(bucket)))
        .collect();
    assert_eq!(
        rates,
        [
            (1, 1600.0, Some(10.0)),
            (1, 1900.0, Some(5.0)),
            (1, 100.0, Some(100.0 / 60.0)),
        ]
    );

    let list = reader
        .list_metrics(
            &parse_query("kind = counter", Signal::Metrics).unwrap(),
            Resolution::Raw,
            10,
        )
        .unwrap();
    assert_eq!(list.series.len(), 1);
    assert_eq!(
        serde_json::to_value(&list.series[0]).unwrap(),
        json!({
            "name": "system.network.io",
            "kind": "counter",
            "temporality": "cumulative",
            "unit": "By",
            "service": "api",
            "labels": {"network.interface.name": "eth0"},
            "resource": {"service.name": "api"},
        })
    );
}

#[test]
fn a_series_past_the_limit_is_cut_by_the_points_of_the_range() {
    let directory = tempfile::tempdir().unwrap();
    let range_start_at = Day::today().start_at() + 3600 * SECOND;
    let queue_lag_metric = |queue: &str, offset: i64| Metric {
        name: "queue.lag".into(),
        unit: "s".into(),
        labels: attributes_from_json(json!({"queue": queue})),
        points: Points::Gauge(vec![NumberPoint {
            recorded_at: range_start_at + offset * SECOND,
            value: 1.0,
        }]),
    };
    let mut api = records("api", &json!({"service.name": "api"}));
    api.metrics = vec![
        queue_lag_metric("ended", -30),
        queue_lag_metric("email", 10),
        queue_lag_metric("sms", 20),
    ];
    write_batch(directory.path(), vec![api], &Indexes::default());
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(range_start_at, range_start_at + 600 * SECOND).unwrap(),
    )
    .unwrap();
    let filter = MetricFilter {
        name: "queue.lag".into(),
        query: parse_query("", Signal::Metrics).unwrap(),
        step_ns: 60 * SECOND,
        resolution: Resolution::Raw,
        grouping: Grouping::default(),
    };
    let limited_to_two = reader.get_metric_series(&filter, 2).unwrap();
    assert_eq!(limited_to_two.groups.len(), 2);
    assert!(!limited_to_two.truncated);
    let limited_to_one = reader.get_metric_series(&filter, 1).unwrap();
    assert_eq!(limited_to_one.groups.len(), 1);
    assert_eq!(
        labels_and_resource_of(&limited_to_one.groups[0]).0["queue"],
        "email"
    );
    assert!(limited_to_one.truncated);
}

#[test]
fn an_exponential_histogram_returns_the_bounds_of_its_buckets() {
    let directory = tempfile::tempdir().unwrap();
    let range_start_at = Day::today().start_at();
    let point = |offset: i64, scale: i32, counts: &[u64]| HistogramPoint {
        recorded_at: range_start_at + offset * SECOND,
        histogram: Histogram {
            count: counts.iter().sum(),
            sum: Some(1.0),
            min: None,
            max: None,
            buckets: Buckets::Exponential(
                ExponentialBuckets::new(
                    scale,
                    0,
                    IndexedCounts {
                        offset: 0,
                        counts: counts.to_vec(),
                    },
                    IndexedCounts::default(),
                )
                .unwrap(),
            ),
        },
    };
    let mut api = records("api", &json!({"service.name": "api"}));
    api.metrics = vec![Metric {
        name: "http.server.request.duration".into(),
        unit: "s".into(),
        labels: Attributes::new(),
        points: Points::Histogram(
            Temporality::Delta,
            vec![point(1, 0, &[10, 10]), point(30, 1, &[2, 2])],
        ),
    }];
    write_batch(directory.path(), vec![api], &Indexes::default());
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(range_start_at, range_start_at + 600 * SECOND).unwrap(),
    )
    .unwrap();
    let filter = MetricFilter {
        name: "http.server.request.duration".into(),
        query: parse_query("", Signal::Metrics).unwrap(),
        step_ns: 60 * SECOND,
        resolution: Resolution::Raw,
        grouping: Grouping::default(),
    };
    let metric = reader.get_metric_series(&filter, 10).unwrap();
    assert_eq!(
        metric.groups[0].kind,
        MetricKind::Histogram(Temporality::Delta)
    );
    let merged = distribution_of(&metric.groups[0].buckets[0]);
    // The point of scale 1 joins its two buckets into the first one of scale 0.
    assert_eq!(merged.bounds, [1.0, 2.0, 4.0]);
    assert_eq!(merged.counts, [0, 14, 10, 0]);
    assert_eq!(merged.count, 24);
}

#[test]
fn a_group_combines_the_series_of_two_resources() {
    let directory = tempfile::tempdir().unwrap();
    let range_start_at = Day::today().start_at() + 3600 * SECOND;
    let memory_usage_metric = |state: &str, value: f64| Metric {
        name: "system.memory.usage".into(),
        unit: "By".into(),
        labels: attributes_from_json(json!({"system.memory.state": state})),
        points: Points::UpDown(vec![NumberPoint {
            recorded_at: range_start_at + 10 * SECOND,
            value,
        }]),
    };
    let host = |name: &str, used: f64, free: f64| {
        let mut host = records("otelo", &json!({"host.name": name}));
        host.metrics = vec![
            memory_usage_metric("used", used),
            memory_usage_metric("free", free),
        ];
        host
    };
    write_batch(
        directory.path(),
        vec![host("droplet", 300.0, 700.0), host("laptop", 500.0, 1500.0)],
        &Indexes::default(),
    );
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(range_start_at, range_start_at + 600 * SECOND).unwrap(),
    )
    .unwrap();
    let by_state = MetricFilter {
        name: "system.memory.usage".into(),
        query: parse_query("", Signal::Metrics).unwrap(),
        step_ns: 60 * SECOND,
        resolution: Resolution::Raw,
        grouping: Grouping {
            by: vec!["system.memory.state".parse().unwrap()],
            top: None,
        },
    };
    let group_totals = |filter: &MetricFilter, limit: usize| {
        let metric = reader.get_metric_series(filter, limit).unwrap();
        let totals: Vec<(Value, f64)> = metric
            .groups
            .iter()
            .map(|group| {
                (
                    serde_json::to_value(&group.key).unwrap(),
                    group.buckets[0].last,
                )
            })
            .collect();
        (totals, metric.truncated)
    };
    assert_eq!(
        group_totals(&by_state, 10),
        (
            vec![
                (
                    json!({"type": "values", "values": {"system.memory.state": "free"}, "series_count": 2}),
                    2200.0
                ),
                (
                    json!({"type": "values", "values": {"system.memory.state": "used"}, "series_count": 2}),
                    800.0
                ),
            ],
            false
        )
    );
    let top_host = MetricFilter {
        grouping: Grouping {
            by: vec!["resource.host.name".parse().unwrap()],
            top: NonZeroUsize::new(1),
        },
        ..by_state.clone()
    };
    assert_eq!(
        group_totals(&top_host, 10),
        (
            vec![
                (
                    json!({"type": "values", "values": {"resource.host.name": "laptop"}, "series_count": 2}),
                    2000.0
                ),
                (
                    json!({"type": "other", "group_count": 1, "series_count": 2}),
                    1000.0
                ),
            ],
            false
        )
    );
    let (first_group, truncated) = group_totals(&by_state, 1);
    assert_eq!(first_group.len(), 1);
    assert!(truncated);
}
