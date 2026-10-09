mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU16;
use std::path::Path;
use std::sync::Arc;

use otelo_indexed_storage::query::{
    GroupBuckets, LogGroupingField, PageRequest, RankOrder, SpanGroupRanking, SpanGroupingField,
    SpanSort,
};
use otelo_indexed_storage::{
    AttributeValue, Attributes, Batch, IndexedAttribute, IndexedSignal, Log, PipelineMeters,
    RangeQueries, Records, Resource, Severity, Span, SpanId, SpanKind, SpanStatus, TimeRange,
    TraceContext, TraceId, now_unix_nanos,
};
use otelo_indexed_storage_sqlite::{
    Config, Day, Indexes, Progress, Reader, TELEMETRY_FILE_NAME, TelemetryFile,
    index_journal_until_caught_up,
};
use otelo_query::{Signal, parse_query};
use rusqlite::Connection;
use serde_json::{Value, json};

const MILLISECOND: i64 = 1_000_000;
const HOUR: i64 = 3_600 * 1_000_000_000;

const QUERY_TEXTS: [&str; 8] = [
    "SELECT count(*) FROM users",
    "SELECT id, name FROM users WHERE id = ?1",
    "SELECT * FROM matches WHERE player_id = ?1 ORDER BY started_at DESC",
    "INSERT INTO answers (question_id, slot) VALUES (?1, ?2)",
    "UPDATE ratings SET rating = ?2 WHERE user_id = ?1",
    "SELECT q.id, q.text FROM questions q JOIN tags t ON t.id = q.tag_id",
    "DELETE FROM sessions WHERE expires_at < ?1",
    "SELECT value FROM settings WHERE key = ?1",
];

fn attributes_from_json(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

fn start_of_today() -> i64 {
    Day::today().start_at()
}

const fn span_id_of(number: u64) -> SpanId {
    SpanId(number.to_be_bytes())
}

fn span(number: u64, name: &str, attributes: Value) -> Span {
    Span {
        trace_id: TraceId([7; 16]),
        span_id: span_id_of(number),
        parent_span_id: None,
        name: name.into(),
        kind: SpanKind::Client,
        started_at: start_of_today() + i64::try_from(number).unwrap() * MILLISECOND,
        duration_ns: 1_000,
        status_code: SpanStatus::Unset,
        attributes: attributes_from_json(attributes),
        events: Vec::new(),
    }
}

fn log(number: i64, scope: &str, attributes: Value) -> Log {
    let mut attributes = attributes_from_json(attributes);
    attributes.insert("otel.scope.name", scope);
    Log {
        logged_at: start_of_today() + number * MILLISECOND,
        severity_number: Severity::INFO,
        body: format!("line {number}"),
        trace_context: TraceContext::None,
        attributes,
    }
}

fn resource() -> Resource {
    Resource {
        service: "mudro".into(),
        attributes: attributes_from_json(json!({"service.name": "mudro"})),
    }
}

fn batch_of_spans(spans: Vec<Span>) -> Batch {
    vec![Records {
        resource: resource(),
        logs: Vec::new(),
        spans,
        metrics: Vec::new(),
    }]
}

fn batch_of_logs(logs: Vec<Log>) -> Batch {
    vec![Records {
        resource: resource(),
        logs,
        spans: Vec::new(),
        metrics: Vec::new(),
    }]
}

fn write_spans(directory: &Path, spans: Vec<Span>) {
    write_spans_with_indexes(directory, spans, &Indexes::default());
}

fn write_spans_with_indexes(directory: &Path, spans: Vec<Span>, indexes: &Indexes) {
    let mut config = Config::new(directory.to_owned());
    config.indexes = indexes.clone();
    common::index_batches_of_signal(config, Signal::Spans, vec![batch_of_spans(spans)]);
}

fn write_logs(directory: &Path, logs: Vec<Log>) {
    common::index_batches_of_signal(
        Config::new(directory.to_owned()),
        Signal::Logs,
        vec![batch_of_logs(logs)],
    );
}

fn open_telemetry_file(directory: &Path) -> Connection {
    Connection::open(directory.join(TELEMETRY_FILE_NAME)).unwrap()
}

fn query_integer(directory: &Path, sql: &str) -> i64 {
    open_telemetry_file(directory)
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

fn read_encodings_of_group(directory: &Path, group_name: &str) -> BTreeMap<String, String> {
    open_telemetry_file(directory)
        .prepare(
            "SELECT attribute_key.key, attribute_key_profile.encoding
             FROM attribute_key_profiles attribute_key_profile
             JOIN attribute_keys attribute_key ON attribute_key.id = attribute_key_profile.attribute_key_id
             JOIN record_groups record_group ON record_group.id = attribute_key_profile.record_group_id
             WHERE record_group.name = ?1",
        )
        .unwrap()
        .query_map([group_name], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn count_stable_sets_of_group(directory: &Path, group_name: &str) -> i64 {
    open_telemetry_file(directory)
        .query_row(
            "SELECT count(*)
             FROM stable_attribute_sets stable_attribute_set
             JOIN record_groups record_group ON record_group.id = stable_attribute_set.record_group_id
             WHERE record_group.name = ?1",
            [group_name],
            |row| row.get(0),
        )
        .unwrap()
}

fn open_reader_of_today(directory: &Path) -> Reader {
    let start_at = start_of_today();
    Reader::open(
        directory,
        TimeRange::new(start_at - HOUR, start_at + 30 * HOUR).unwrap(),
    )
    .unwrap()
}

fn read_spans_by_id(reader: &Reader, query: &str) -> BTreeMap<SpanId, Attributes> {
    let mut spans = BTreeMap::new();
    let query = parse_query(query, Signal::Spans).unwrap();
    let mut page = PageRequest::first(1_000);
    loop {
        let read = reader.list_spans(&query, SpanSort::Oldest, &page).unwrap();
        for span in read.spans {
            spans.insert(span.span_id, span.attributes);
        }
        let Some(next) = read.next else {
            return spans;
        };
        page = PageRequest {
            after: Some(next),
            ..PageRequest::first(1_000)
        };
    }
}

// The spans of mudro: SQL statements whose code location and timings repeat, and a timer whose
// timings and match differ.
fn build_mudro_like_spans() -> Vec<Span> {
    let mut spans = Vec::new();
    for number in 0..600_u64 {
        let index = usize::try_from(number).unwrap();
        spans.push(span(
            number,
            "SELECT",
            json!({
                "busy_ns": 0,
                "idle_ns": number * 7_919 % 100_003,
                "code.file.path": "crates/mudro-storage-sqlite/src/statements.rs",
                "code.line.number": 37,
                "db.system.name": "sqlite",
                "db.operation.name": "SELECT",
                "db.query.text": QUERY_TEXTS[index % QUERY_TEXTS.len()],
                "thread.id": number % 2,
                "thread.name": "tokio-rt-worker",
                "user.name": format!("user-{}", number % 100),
            }),
        ));
    }
    for number in 600..1_000_u64 {
        let busy_ns = if number % 10 == 0 { 0 } else { number * 131 };
        spans.push(span(
            number,
            "match woke",
            json!({
                "busy_ns": busy_ns,
                "idle_ns": number * 17,
                "code.file.path": "crates/mudro-server/src/match_driver.rs",
                "code.line.number": 427,
                "match_id": 64,
                "thread.name": "tokio-rt-worker",
            }),
        ));
    }
    spans
}

fn collect_attributes_by_span_id(spans: &[Span]) -> BTreeMap<SpanId, Attributes> {
    spans
        .iter()
        .map(|span| (span.span_id, span.attributes.clone()))
        .collect()
}

#[test]
fn each_group_keeps_its_repeated_keys_in_a_stable_set() {
    let directory = tempfile::tempdir().unwrap();
    write_spans(directory.path(), build_mudro_like_spans());

    let encodings = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|&(key, encoding)| (key.to_owned(), encoding.to_owned()))
            .collect()
    };
    assert_eq!(
        read_encodings_of_group(directory.path(), "SELECT"),
        encodings(&[
            ("busy_ns", "stable"),
            ("code.file.path", "stable"),
            ("code.line.number", "stable"),
            ("db.operation.name", "stable"),
            ("db.query.text", "stable"),
            ("db.system.name", "stable"),
            ("idle_ns", "literal"),
            ("thread.id", "stable"),
            ("thread.name", "stable"),
            ("user.name", "interned"),
        ])
    );
    assert_eq!(
        read_encodings_of_group(directory.path(), "match woke"),
        encodings(&[
            ("busy_ns", "literal"),
            ("code.file.path", "stable"),
            ("code.line.number", "stable"),
            ("idle_ns", "literal"),
            ("match_id", "stable"),
            ("thread.name", "stable"),
        ])
    );
    // One set for each of the first 32 spans, then eight statements on alternating threads.
    assert!(
        count_stable_sets_of_group(directory.path(), "SELECT") <= 32 + 16,
        "{}",
        count_stable_sets_of_group(directory.path(), "SELECT")
    );
    assert_eq!(
        query_integer(
            directory.path(),
            "SELECT count(*) FROM interned_attribute_values"
        ),
        89 + 8,
        "the user names from the 512th span on, and the statements from the 32nd to the 511th"
    );
}

#[test]
fn every_span_reads_back_with_the_attributes_it_was_sent_with() {
    let directory = tempfile::tempdir().unwrap();
    let spans = build_mudro_like_spans();
    write_spans(directory.path(), spans.clone());
    assert_eq!(
        read_spans_by_id(&open_reader_of_today(directory.path()), ""),
        collect_attributes_by_span_id(&spans)
    );
}

#[test]
fn every_kind_of_value_reads_back_in_every_encoding() {
    let directory = tempfile::tempdir().unwrap();
    let odd_values = json!({
        "null": null,
        "bool": true,
        "int": -9_007_199_254_740_993_i64,
        "double": 0.1,
        "string": "naïve \"quoted\" \\ text 🙂",
        "array": [1, "two", [3.5], {"four": null}],
        "map": {"nested.key": {"deeper": [false]}},
        "say \"hi\"": "a key with quotes",
        "dotted.key.name": "",
    });
    let spans: Vec<Span> = (0..300_u64)
        .map(|number| {
            let mut attributes = odd_values.clone();
            attributes["varying string"] = json!(format!("value {number}"));
            attributes["varying double"] = json!(f64::from(u32::try_from(number).unwrap()) / 3.0);
            span(number, "odd values", attributes)
        })
        .collect();
    write_spans(directory.path(), spans.clone());
    let encodings = read_encodings_of_group(directory.path(), "odd values");
    assert_eq!(encodings["map"], "stable");
    assert_eq!(encodings["varying string"], "literal");
    assert_eq!(encodings["varying double"], "literal");
    assert_eq!(
        read_spans_by_id(&open_reader_of_today(directory.path()), ""),
        collect_attributes_by_span_id(&spans)
    );
}

type Oracle = fn(&Attributes) -> bool;

fn find_value<'a>(attributes: &'a Attributes, key: &str) -> Option<&'a AttributeValue> {
    attributes.get(key)
}

fn find_integer(attributes: &Attributes, key: &str) -> Option<i64> {
    match find_value(attributes, key)? {
        AttributeValue::Int(integer) => Some(*integer),
        _ => None,
    }
}

fn find_text<'a>(attributes: &'a Attributes, key: &str) -> Option<&'a str> {
    find_value(attributes, key)?.as_str()
}

const SPAN_QUERIES_WITH_ORACLES: &[(&str, Oracle)] = &[
    (
        r#"db.query.text = "SELECT count(*) FROM users""#,
        |attributes| find_text(attributes, "db.query.text") == Some(QUERY_TEXTS[0]),
    ),
    (
        r#"db.query.text != "SELECT count(*) FROM users""#,
        |attributes| find_text(attributes, "db.query.text") != Some(QUERY_TEXTS[0]),
    ),
    (r#"db.query.text = "never sent""#, |_| false),
    (r#"db.query.text ~ "FROM users""#, |attributes| {
        find_text(attributes, "db.query.text").is_some_and(|text| text.contains("FROM users"))
    }),
    ("busy_ns = 0", |attributes| {
        find_integer(attributes, "busy_ns") == Some(0)
    }),
    ("busy_ns != 0", |attributes| {
        find_integer(attributes, "busy_ns") != Some(0)
    }),
    ("busy_ns > 50000", |attributes| {
        find_integer(attributes, "busy_ns").is_some_and(|busy_ns| busy_ns > 50_000)
    }),
    ("idle_ns <= 1000", |attributes| {
        find_integer(attributes, "idle_ns").is_some_and(|idle_ns| idle_ns <= 1_000)
    }),
    (r#"code.line.number = "37""#, |attributes| {
        find_integer(attributes, "code.line.number") == Some(37)
    }),
    (
        r#"user.name in ("user-1", "user-2", "nobody")"#,
        |attributes| {
            matches!(
                find_text(attributes, "user.name"),
                Some("user-1" | "user-2")
            )
        },
    ),
    (r#"user.name ~ "er-9""#, |attributes| {
        find_text(attributes, "user.name").is_some_and(|name| name.contains("er-9"))
    }),
    (r#"user.name > "user-95""#, |attributes| {
        find_text(attributes, "user.name").is_some_and(|name| name > "user-95")
    }),
    ("has(match_id)", |attributes| {
        find_value(attributes, "match_id").is_some()
    }),
    ("NOT has(match_id)", |attributes| {
        find_value(attributes, "match_id").is_none()
    }),
    ("has(no.such.key)", |_| false),
    ("NOT no.such.key = 1", |_| true),
    ("thread.id = 1 AND busy_ns = 0", |attributes| {
        find_integer(attributes, "thread.id") == Some(1)
            && find_integer(attributes, "busy_ns") == Some(0)
    }),
    ("thread.id = 1 OR match_id = 64", |attributes| {
        find_integer(attributes, "thread.id") == Some(1)
            || find_integer(attributes, "match_id") == Some(64)
    }),
    (r#"name = "match woke" AND busy_ns = 0"#, |attributes| {
        find_integer(attributes, "match_id").is_some()
            && find_integer(attributes, "busy_ns") == Some(0)
    }),
];

#[test]
fn a_query_matches_the_same_spans_whatever_the_encodings_of_its_keys() {
    let directory = tempfile::tempdir().unwrap();
    let spans = build_mudro_like_spans();
    write_spans(directory.path(), spans.clone());
    let reader = open_reader_of_today(directory.path());
    for &(query, oracle) in SPAN_QUERIES_WITH_ORACLES {
        let expected: BTreeSet<SpanId> = spans
            .iter()
            .filter(|span| oracle(&span.attributes))
            .map(|span| span.span_id)
            .collect();
        let matched: BTreeSet<SpanId> = read_spans_by_id(&reader, query).into_keys().collect();
        assert_eq!(matched, expected, "{query}");
    }
}

#[test]
fn spans_group_by_a_key_whatever_its_encoding() {
    let directory = tempfile::tempdir().unwrap();
    let spans = build_mudro_like_spans();
    write_spans(directory.path(), spans.clone());
    let reader = open_reader_of_today(directory.path());
    for key in ["db.query.text", "busy_ns", "user.name", "match_id"] {
        let mut expected: BTreeMap<Option<String>, u64> = BTreeMap::new();
        for span in &spans {
            *expected
                .entry(span.attributes.get(key).map(ToString::to_string))
                .or_default() += 1;
        }
        let groups = reader
            .list_span_groups(
                &parse_query("", Signal::Spans).unwrap(),
                &[SpanGroupingField::Attribute(key.to_owned())],
                SpanGroupRanking::default(),
                HOUR,
                GroupBuckets::Omitted,
                10_000,
            )
            .unwrap();
        let grouped: BTreeMap<Option<String>, u64> = groups
            .groups
            .iter()
            .map(|group| {
                (
                    group.values.get(key).map(ToString::to_string),
                    group.spans.count,
                )
            })
            .collect();
        assert_eq!(grouped, expected, "{key}");
    }
}

#[test]
fn a_stable_key_that_starts_to_vary_stops_making_stable_sets() {
    let directory = tempfile::tempdir().unwrap();
    let spans: Vec<Span> = (0..1_000_u64)
        .map(|number| {
            let match_id = if number < 300 { 1 } else { number };
            span(
                number,
                "match woke",
                json!({"match_id": match_id, "code.line.number": 427}),
            )
        })
        .collect();
    write_spans(directory.path(), spans.clone());

    assert_eq!(
        read_encodings_of_group(directory.path(), "match woke")["match_id"],
        "literal"
    );
    let stable_sets = count_stable_sets_of_group(directory.path(), "match woke");
    assert!(
        stable_sets < 200,
        "a reclassification past 128 values ends the sets, not one per match: {stable_sets}"
    );
    let reader = open_reader_of_today(directory.path());
    assert_eq!(read_spans_by_id(&reader, "match_id = 1").len(), 300);
    assert_eq!(read_spans_by_id(&reader, "match_id = 777").len(), 1);
    assert_eq!(
        read_spans_by_id(&reader, ""),
        collect_attributes_by_span_id(&spans)
    );
}

#[test]
fn a_stable_key_stays_stable_below_twice_the_values_it_took_to_become_stable() {
    let directory = tempfile::tempdir().unwrap();
    let spans: Vec<Span> = (0..4_096_u64)
        .map(|number| {
            let route = if number < 2_048 {
                number % 50
            } else {
                number % 100
            };
            span(
                number,
                "GET",
                json!({"http.route": format!("/route/{route}")}),
            )
        })
        .collect();
    write_spans(directory.path(), spans);
    let (encoding, distinct_value_count): (String, i64) = open_telemetry_file(directory.path())
        .query_row(
            "SELECT attribute_key_profile.encoding, attribute_key_profile.distinct_value_count
             FROM attribute_key_profiles attribute_key_profile
             JOIN attribute_keys attribute_key ON attribute_key.id = attribute_key_profile.attribute_key_id
             WHERE attribute_key.key = 'http.route'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(encoding, "stable");
    assert!(
        (80..=128).contains(&distinct_value_count),
        "about 100 routes: {distinct_value_count}"
    );
}

#[test]
fn a_group_keeps_stable_only_the_keys_whose_combinations_stay_few() {
    let directory = tempfile::tempdir().unwrap();
    let spans: Vec<Span> = (0..2_048_u64)
        .map(|number| {
            span(
                number,
                "GET",
                json!({
                    "first": number % 30,
                    "second": number * 7 % 30,
                    "third": number * 11 / 30 % 30,
                }),
            )
        })
        .collect();
    write_spans(directory.path(), spans.clone());
    let stable_keys = read_encodings_of_group(directory.path(), "GET")
        .into_values()
        .filter(|encoding| encoding == "stable")
        .count();
    assert_eq!(stable_keys, 2);
    assert_eq!(
        read_spans_by_id(&open_reader_of_today(directory.path()), ""),
        collect_attributes_by_span_id(&spans)
    );
}

#[test]
fn a_restarted_indexer_stores_with_the_profiles_it_left() {
    let directory = tempfile::tempdir().unwrap();
    let spans = build_mudro_like_spans();
    write_spans(directory.path(), spans[..600].to_vec());
    let count_rows = || {
        (
            query_integer(
                directory.path(),
                "SELECT count(*) FROM stable_attribute_sets",
            ),
            query_integer(
                directory.path(),
                "SELECT count(*) FROM interned_attribute_values",
            ),
        )
    };
    let before = count_rows();
    let later_spans: Vec<Span> = (580..600_u64)
        .map(|number| {
            let mut later = spans[usize::try_from(number).unwrap()].clone();
            later.span_id = span_id_of(10_000 + number);
            later
        })
        .collect();
    write_spans(directory.path(), later_spans);
    assert_eq!(count_rows(), before);
}

#[test]
fn an_hour_of_frames_classifies_a_quiet_group() {
    let directory = tempfile::tempdir().unwrap();
    let journal_directory = tempfile::tempdir().unwrap();
    let opened = common::open_journal(journal_directory.path());
    let now = now_unix_nanos();
    let batches: Vec<Batch> = (0..11_u64)
        .map(|number| batch_of_spans(vec![span(number, "GET", json!({"http.route": "/"}))]))
        .collect();
    let frames: Vec<(u32, i64)> = (0..11_u32)
        .map(|number| {
            let received_at = if number < 10 { now - 2 * HOUR } else { now };
            (number, received_at)
        })
        .collect();
    common::append_numbered_frames(opened.journal.as_ref(), Signal::Spans, &frames);
    index_journal_until_caught_up(
        Config::new(directory.path().to_owned()),
        opened.journal,
        common::map_numbered_frames(batches),
        Arc::new(PipelineMeters::default()),
        |_, _| {},
    )
    .unwrap();
    opened.threads.stop_and_join().unwrap();
    assert_eq!(
        query_integer(
            directory.path(),
            "SELECT record_count FROM attribute_key_profiles"
        ),
        11
    );
}

#[test]
fn logs_of_each_scope_are_profiled_apart() {
    let directory = tempfile::tempdir().unwrap();
    let logs: Vec<Log> = (0..200_i64)
        .flat_map(|number| {
            [
                log(2 * number, "billing", json!({"tenant": "acme"})),
                log(
                    2 * number + 1,
                    "matches",
                    json!({"tenant": format!("t{number}")}),
                ),
            ]
        })
        .collect();
    write_logs(directory.path(), logs.clone());
    assert_eq!(
        read_encodings_of_group(directory.path(), "billing")["tenant"],
        "stable"
    );
    assert_eq!(
        read_encodings_of_group(directory.path(), "matches")["tenant"],
        "literal"
    );
    let reader = open_reader_of_today(directory.path());
    let read_bodies = |query: &str| -> BTreeSet<String> {
        reader
            .list_logs(
                &parse_query(query, Signal::Logs).unwrap(),
                &PageRequest::first(1_000),
            )
            .unwrap()
            .logs
            .into_iter()
            .map(|line| line.body)
            .collect()
    };
    assert_eq!(read_bodies(r#"tenant = "acme""#).len(), 200);
    assert_eq!(
        read_bodies(r#"tenant = "t7""#),
        BTreeSet::from(["line 15".to_owned()])
    );
    let all_logs = reader
        .list_logs(
            &parse_query("", Signal::Logs).unwrap(),
            &PageRequest::first(1_000),
        )
        .unwrap()
        .logs;
    let read_attributes: BTreeMap<String, Attributes> = all_logs
        .into_iter()
        .map(|line| (line.body, line.attributes))
        .collect();
    let sent_attributes: BTreeMap<String, Attributes> = logs
        .into_iter()
        .map(|log| (log.body, log.attributes))
        .collect();
    assert_eq!(read_attributes, sent_attributes);
    let counts = reader
        .count_logs(
            &parse_query("", Signal::Logs).unwrap(),
            &[LogGroupingField::Attribute("tenant".to_owned())],
            RankOrder::Highest,
            HOUR,
            1,
        )
        .unwrap();
    assert_eq!(counts.groups[0].count, 200);
    assert_eq!(
        counts.groups[0].values["tenant"],
        AttributeValue::from("acme")
    );
}

#[test]
fn a_stable_key_needs_no_index_and_an_interned_one_uses_its_own() {
    let directory = tempfile::tempdir().unwrap();
    let indexes = Indexes::new(BTreeSet::from([IndexedAttribute::new(
        IndexedSignal::Spans,
        "user.name",
    )
    .unwrap()]));
    write_spans_with_indexes(directory.path(), build_mudro_like_spans(), &indexes);
    let mut reader = open_reader_of_today(directory.path());
    reader.set_indexed_attributes(indexes.attributes());
    let explain = |query: &str| {
        reader
            .explain_query(&parse_query(query, Signal::Spans).unwrap())
            .unwrap()
    };
    let plan = explain(r#"db.system.name = "sqlite""#);
    assert!(
        plan.iter()
            .any(|step| step.contains("spans_stable_attribute_set_id_started_at")),
        "{plan:?}"
    );
    let plan = explain(r#"user.name = "user-50""#);
    assert!(
        plan.iter()
            .any(|step| step.contains("spans_interned_attribute_")),
        "{plan:?}"
    );
    assert!(
        plan.iter()
            .any(|step| step.contains("spans_literal_attribute_")),
        "the user names between the 32nd and the 511th span are on their rows: {plan:?}"
    );
    let unindexed = reader
        .list_spans(
            &parse_query(
                r#"db.system.name = "sqlite" AND idle_ns = 3"#,
                Signal::Spans,
            )
            .unwrap(),
            SpanSort::Newest,
            &PageRequest::first(1),
        )
        .unwrap()
        .unindexed;
    assert_eq!(unindexed, ["idle_ns"]);
}

#[test]
fn retention_deletes_the_sets_and_values_that_only_old_records_have() {
    let directory = tempfile::tempdir().unwrap();
    let three_days_ago = Day::today().add_days(-3).start_at() + HOUR;
    let span_at = |number: u64, started_at: i64, name: &str, attributes: Value| {
        let mut sent = span(number, name, attributes);
        sent.started_at = started_at;
        sent
    };
    let mut spans: Vec<Span> = (0..40_u64)
        .map(|number| {
            span_at(
                number,
                three_days_ago,
                "old job",
                json!({"job": "nightly", "note": format!("old only {}", number % 10)}),
            )
        })
        .collect();
    spans.extend((40..80_u64).map(|number| {
        span_at(
            number,
            three_days_ago,
            "GET",
            json!({"note": format!("shared {}", number % 10)}),
        )
    }));
    spans.extend((80..120_u64).map(|number| {
        span_at(
            number,
            start_of_today() + i64::try_from(number).unwrap(),
            "GET",
            json!({"note": format!("shared {}", number % 10)}),
        )
    }));
    write_spans(directory.path(), spans.clone());
    assert!(!read_encodings_of_group(directory.path(), "old job").is_empty());
    let count_values_like = |pattern: &str| {
        open_telemetry_file(directory.path())
            .query_row(
                "SELECT count(*) FROM interned_attribute_values WHERE value LIKE ?1",
                [pattern],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    assert!(count_values_like("\"old only%") > 0);
    let stable_sets_before = query_integer(
        directory.path(),
        "SELECT count(*) FROM stable_attribute_sets",
    );

    // Logs and spans share the interned values, which stay for the longer of their retentions.
    let mut config = Config::new(directory.path().to_owned());
    config.traces_retention_days = NonZeroU16::new(2).unwrap();
    config.logs_retention_days = NonZeroU16::new(2).unwrap();
    let mut file = TelemetryFile::open(directory.path()).unwrap();
    while file
        .delete_next_past_retention(config.oldest_retained_days(Day::today()))
        .unwrap()
        == Progress::MoreIsDue
    {}
    drop(file);

    assert_eq!(count_values_like("\"old only%"), 0);
    assert_eq!(count_values_like("\"shared%"), 10);
    assert!(
        query_integer(
            directory.path(),
            "SELECT count(*) FROM stable_attribute_sets"
        ) < stable_sets_before
    );
    assert!(read_encodings_of_group(directory.path(), "old job").is_empty());
    assert_eq!(count_stable_sets_of_group(directory.path(), "old job"), 0);
    assert_eq!(
        read_spans_by_id(&open_reader_of_today(directory.path()), ""),
        collect_attributes_by_span_id(&spans[80..])
    );
}

// mudro's spans as JSON on the row took about 810 bytes each.
#[test]
fn the_attributes_of_repetitive_spans_take_a_fraction_of_their_json() {
    let directory = tempfile::tempdir().unwrap();
    let spans = build_mudro_like_spans();
    let json_bytes: usize = spans
        .iter()
        .map(|span| span.attributes.to_json().len())
        .sum();
    write_spans(directory.path(), spans);
    let stored_bytes = query_integer(
        directory.path(),
        "SELECT (SELECT sum(length(interned_attributes) + length(literal_attributes)) FROM spans)
              + (SELECT sum(length(attributes)) FROM stable_attribute_sets)
              + (SELECT sum(length(value)) FROM interned_attribute_values)",
    );
    let stored_share = f64::from(u32::try_from(stored_bytes).unwrap())
        / f64::from(u32::try_from(json_bytes).unwrap());
    assert!(
        stored_share < 0.2,
        "{stored_bytes} bytes stored for {json_bytes} bytes of JSON: {stored_share:.2}"
    );
}

#[test]
fn a_double_reads_back_to_the_last_bit() {
    let directory = tempfile::tempdir().unwrap();
    let thirds = |number: u32| json!({"third": f64::from(number) / 3.0});
    let spans: Vec<Span> = (0..300_u32)
        .map(|number| span(u64::from(number), "thirds", thirds(number)))
        .collect();
    write_spans(directory.path(), spans.clone());
    let logs: Vec<Log> = (0..300_u32)
        .map(|number| log(i64::from(number), "thirds", thirds(number)))
        .collect();
    write_logs(directory.path(), logs.clone());

    let reader = open_reader_of_today(directory.path());
    assert_eq!(
        read_spans_by_id(&reader, ""),
        collect_attributes_by_span_id(&spans)
    );
    let read_logs: BTreeMap<String, Attributes> = reader
        .list_logs(
            &parse_query("", Signal::Logs).unwrap(),
            &PageRequest::first(1_000),
        )
        .unwrap()
        .logs
        .into_iter()
        .map(|line| (line.body, line.attributes))
        .collect();
    let sent_logs: BTreeMap<String, Attributes> = logs
        .into_iter()
        .map(|log| (log.body, log.attributes))
        .collect();
    assert_eq!(read_logs, sent_logs);
}
