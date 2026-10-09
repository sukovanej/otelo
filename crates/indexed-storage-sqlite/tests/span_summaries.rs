mod common;

use otelo_indexed_storage::query::{
    GroupBuckets, PageRequest, SpanGroupRanking, SpanGroupingField, SpanSort,
};
use otelo_indexed_storage::{
    AttributeValue, Attributes, RangeQueries, Records, Resource, Span, SpanId, SpanKind,
    SpanStatus, TimeRange, TraceId,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader};
use otelo_query::{Signal, parse_query};
use serde_json::{Value, json};

const SECOND: i64 = 1_000_000_000;
const MILLISECOND: i64 = 1_000_000;
const MINUTE: i64 = 60 * SECOND;

fn attributes_from_json(value: &Value) -> Attributes {
    serde_json::from_value(value.clone()).unwrap()
}

fn span(span_id_byte: u8, started_at: i64, attributes: &Value) -> Span {
    Span {
        trace_id: TraceId([span_id_byte; 16]),
        span_id: SpanId([span_id_byte; 8]),
        parent_span_id: None,
        name: "GET".into(),
        kind: SpanKind::Server,
        started_at,
        duration_ns: 10 * MILLISECOND,
        status_code: SpanStatus::Unset,
        attributes: attributes_from_json(attributes),
        events: Vec::new(),
    }
}

fn write_spans(directory: &std::path::Path, spans: Vec<Span>) {
    common::index_batches(
        Config::new(directory.to_owned(), common::INDEX_RETENTION_DAYS),
        vec![vec![Records {
            resource: Resource {
                service: "api".into(),
                attributes: Attributes::new(),
            },
            logs: Vec::new(),
            spans,
            metrics: Vec::new(),
        }]],
    );
}

fn open_reader(directory: &std::path::Path, start_at: i64, end_at: i64) -> Reader {
    Reader::open(directory, TimeRange::new(start_at, end_at).unwrap()).unwrap()
}

fn count_spans_by(reader: &Reader, query: &str, key: &str) -> Vec<(Option<AttributeValue>, u64)> {
    let groups = reader
        .list_span_groups(
            &parse_query(query, Signal::Spans).unwrap(),
            &[SpanGroupingField::Attribute(key.into())],
            SpanGroupRanking::default(),
            MINUTE,
            GroupBuckets::Omitted,
            10,
        )
        .unwrap();
    let mut counts: Vec<_> = groups
        .groups
        .iter()
        .map(|group| (group.values.get(key).cloned(), group.spans.count))
        .collect();
    counts.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| format!("{a:?}").cmp(&format!("{b:?}")))
    });
    counts
}

fn text(value: &str) -> AttributeValue {
    AttributeValue::String(value.into())
}

#[test]
fn a_range_that_cuts_a_minute_counts_only_its_own_spans_of_that_minute() {
    let directory = tempfile::tempdir().unwrap();
    let minute_at = Day::today().start_at() + 10 * MINUTE;
    let route = json!({"http.request.method": "GET", "http.route": "/users"});
    write_spans(
        directory.path(),
        vec![
            span(1, minute_at + 10 * SECOND, &route),
            span(2, minute_at + 50 * SECOND, &route),
            span(3, minute_at + MINUTE + 10 * SECOND, &route),
            span(4, minute_at + 2 * MINUTE + 10 * SECOND, &route),
            span(5, minute_at + 2 * MINUTE + 50 * SECOND, &route),
        ],
    );
    let reader = open_reader(
        directory.path(),
        minute_at + 30 * SECOND,
        minute_at + 2 * MINUTE + 30 * SECOND,
    );
    assert_eq!(
        count_spans_by(&reader, "kind = server", "http.route"),
        [(Some(text("/users")), 3)]
    );
    let inside_one_minute = open_reader(
        directory.path(),
        minute_at + 5 * SECOND,
        minute_at + 15 * SECOND,
    );
    assert_eq!(
        count_spans_by(&inside_one_minute, "kind = server", "http.route"),
        [(Some(text("/users")), 1)]
    );
}

#[test]
fn a_query_of_an_attribute_the_summaries_leave_out_reads_the_spans() {
    let directory = tempfile::tempdir().unwrap();
    let at = Day::today().start_at() + 10 * MINUTE;
    write_spans(
        directory.path(),
        vec![
            span(1, at, &json!({"http.route": "/users", "user.id": 7})),
            span(2, at, &json!({"http.route": "/users", "user.id": 7})),
            span(3, at, &json!({"http.route": "/users", "user.id": 8})),
            span(4, at, &json!({"http.route": "/orders"})),
        ],
    );
    let reader = open_reader(directory.path(), at - MINUTE, at + MINUTE);
    assert_eq!(
        count_spans_by(&reader, "", "user.id"),
        [
            (None, 1),
            (Some(AttributeValue::Int(8)), 1),
            (Some(AttributeValue::Int(7)), 2)
        ]
    );
    assert_eq!(
        count_spans_by(&reader, "user.id = 7", "http.route"),
        [(Some(text("/users")), 2)]
    );
    assert_eq!(
        count_spans_by(&reader, "has(user.id)", "http.route"),
        [(Some(text("/users")), 3)]
    );
}

#[test]
fn not_equal_keeps_the_spans_without_the_attribute() {
    let directory = tempfile::tempdir().unwrap();
    let at = Day::today().start_at() + 10 * MINUTE;
    write_spans(
        directory.path(),
        vec![
            span(1, at, &json!({"http.route": "/users"})),
            span(2, at, &json!({"http.route": "/orders"})),
            span(3, at, &json!({})),
        ],
    );
    let reader = open_reader(directory.path(), at - MINUTE, at + MINUTE);
    assert_eq!(
        count_spans_by(&reader, "http.route != /users", "http.route"),
        [(None, 1), (Some(text("/orders")), 1)]
    );
}

#[test]
fn the_query_text_of_a_sql_span_keeps_no_literal() {
    let directory = tempfile::tempdir().unwrap();
    let at = Day::today().start_at() + 10 * MINUTE;
    write_spans(
        directory.path(),
        vec![
            span(
                1,
                at,
                &json!({"db.system.name": "sqlite", "db.query.text": "SELECT * FROM t WHERE id IN (1, 2)"}),
            ),
            span(
                2,
                at,
                &json!({"db.system.name": "sqlite", "db.query.text": "SELECT * FROM t WHERE id IN (3)"}),
            ),
            span(
                3,
                at,
                &json!({"db.system.name": "redis", "db.query.text": "GET user:7"}),
            ),
        ],
    );
    let reader = open_reader(directory.path(), at - MINUTE, at + MINUTE);
    assert_eq!(
        count_spans_by(&reader, "", "db.query.text"),
        [
            (Some(text("GET user:7")), 1),
            (Some(text("SELECT * FROM t WHERE id IN (?)")), 2)
        ]
    );
}

#[test]
fn the_failed_traces_of_a_service_come_from_its_failed_spans() {
    let directory = tempfile::tempdir().unwrap();
    let at = Day::today().start_at() + 10 * MINUTE;
    let route = json!({"http.route": "/users"});
    write_spans(
        directory.path(),
        vec![
            span(1, at, &route),
            Span {
                status_code: SpanStatus::Error,
                ..span(2, at + SECOND, &route)
            },
        ],
    );
    let reader = open_reader(directory.path(), at - MINUTE, at + MINUTE);
    let traces = |query: &str| {
        reader
            .list_traces(
                &parse_query(query, Signal::Spans).unwrap(),
                SpanSort::Newest,
                &PageRequest::first(10),
            )
            .unwrap()
            .traces
            .iter()
            .map(|trace| trace.trace_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(traces("service = api error = true"), [TraceId([2; 16])]);
    assert_eq!(traces("service = web error = true"), []);
}
