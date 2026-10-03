mod common;

use std::sync::{Arc, Mutex};

use otelo_indexed_storage::query::{Resolution, SpanGroupingField, SpanSort};
use otelo_indexed_storage::{
    Attributes, Log, RangeQueries, Records, Resource, Severity, Span, SpanId, SpanKind, SpanStatus,
    TimeRange, TraceContext, TraceId,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader};
use otelo_query::{Signal, parse_query};
use tracing::field::{Field, Visit};
use tracing::span;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

const SECOND: i64 = 1_000_000_000;
const MINUTE: i64 = 60 * SECOND;

#[derive(Clone, Default)]
struct StatementTexts(Arc<Mutex<Vec<String>>>);

struct QueryTextVisitor<'a>(&'a mut Vec<String>);

impl Visit for QueryTextVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "db.query.text" {
            self.0.push(value.to_owned());
        }
    }

    fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
}

impl<S: tracing::Subscriber> Layer<S> for StatementTexts {
    fn on_new_span(&self, attributes: &span::Attributes<'_>, _: &span::Id, _: Context<'_, S>) {
        attributes.record(&mut QueryTextVisitor(&mut self.0.lock().unwrap()));
    }
}

fn record_statement_texts(run_queries: impl FnOnce()) -> Vec<String> {
    let texts = StatementTexts::default();
    let subscriber = Registry::default().with(texts.clone());
    tracing::subscriber::with_default(subscriber, run_queries);
    let recorded = texts.0.lock().unwrap().clone();
    assert!(!recorded.is_empty());
    recorded
}

fn root_span(trace_id_byte: u8, kind: SpanKind, name: &str, started_at: i64) -> Span {
    Span {
        trace_id: TraceId([trace_id_byte; 16]),
        span_id: SpanId([trace_id_byte; 8]),
        parent_span_id: None,
        name: name.into(),
        kind,
        started_at,
        duration_ns: 10_000_000,
        status_code: SpanStatus::Unset,
        attributes: Attributes::new(),
        events: Vec::new(),
    }
}

fn log(logged_at: i64, severity: Severity, body: &str) -> Log {
    Log {
        logged_at,
        severity_number: severity,
        body: body.into(),
        trace_context: TraceContext::None,
        attributes: Attributes::new(),
    }
}

fn open_reader_over_records() -> (tempfile::TempDir, Reader) {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    let records = Records {
        resource: Resource {
            service: "api".into(),
            attributes: Attributes::new(),
        },
        logs: vec![
            log(today_start_at + SECOND, Severity::INFO, "started"),
            log(today_start_at + 2 * SECOND, Severity::WARN, "slow"),
            log(today_start_at + 3 * SECOND, Severity::ERROR, "failed"),
        ],
        spans: vec![
            root_span(1, SpanKind::Server, "GET /users", today_start_at + SECOND),
            root_span(
                2,
                SpanKind::Server,
                "POST /orders",
                today_start_at + 2 * SECOND,
            ),
            root_span(3, SpanKind::Client, "SELECT", today_start_at + 3 * SECOND),
        ],
        metrics: Vec::new(),
    };
    common::index_batches(
        Config::new(directory.path().to_owned()),
        vec![vec![records]],
    );
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(today_start_at, today_start_at + MINUTE).unwrap(),
    )
    .unwrap();
    (directory, reader)
}

fn run_queries(reader: &Reader, level: &str, kind: &str, limit: usize) {
    let logs = parse_query(&format!("level >= {level}"), Signal::Logs).unwrap();
    reader.list_logs(&logs, limit).unwrap();
    let spans = parse_query(&format!("kind = {kind}"), Signal::Spans).unwrap();
    reader.list_spans(&spans, SpanSort::Newest, limit).unwrap();
    reader.list_traces(&spans, SpanSort::Newest, limit).unwrap();
    let all_spans = parse_query("", Signal::Spans).unwrap();
    reader
        .list_span_groups(&all_spans, &[SpanGroupingField::Name], MINUTE, limit)
        .unwrap();
    reader.get_trace(TraceId([1; 16]), limit).unwrap();
    let metrics = parse_query("", Signal::Metrics).unwrap();
    reader
        .list_metrics(&metrics, Resolution::Raw, limit)
        .unwrap();
}

#[test]
fn a_statement_has_the_same_text_whatever_values_it_reads() {
    let (_directory, reader) = open_reader_over_records();
    let few = record_statement_texts(|| run_queries(&reader, "error", "client", 1));
    let many = record_statement_texts(|| run_queries(&reader, "info", "server", 3));
    assert_eq!(few, many);
}
