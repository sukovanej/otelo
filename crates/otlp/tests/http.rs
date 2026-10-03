mod common;

use std::io::Write;

use std::sync::Arc;

use common::{Receiver, TestJournal, open_telemetry_file, query_first_column};
use flate2::Compression;
use flate2::write::GzEncoder;
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use otelo_indexed_storage::now_unix_nanos;
use otelo_journal::{Frames, Journal, Position, SyncTicket};
use otelo_query::Signal;
use prost::Message;
use serde_json::{Value, json};

fn post_body(
    receiver: &Receiver,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> (u16, Value) {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent.post(receiver.http_url(path));
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = request.send(body).unwrap();
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap();
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::String(text)),
    )
}

fn post_json(receiver: &Receiver, path: &str, body: &Value) -> (u16, Value) {
    let body = serde_json::to_vec(body).unwrap();
    post_body(
        receiver,
        path,
        &[("Content-Type", "application/json")],
        &body,
    )
}

fn traces_request(resource_attributes: &Value, spans: &Value) -> Value {
    json!({"resourceSpans": [{
        "resource": {"attributes": resource_attributes},
        "scopeSpans": [{"spans": spans}],
    }]})
}

fn span_json(name: &str, trace_id: &str, span_id: &str) -> Value {
    let started_at = now_unix_nanos();
    json!({
        "traceId": trace_id,
        "spanId": span_id,
        "name": name,
        "kind": 2,
        "startTimeUnixNano": started_at.to_string(),
        "endTimeUnixNano": (started_at + 1_000_000).to_string(),
    })
}

const TRACE_ID_HEX: &str = "5b8efff798038103d269b633813fc60c";
const SPAN_ID_HEX: &str = "eee19b7ec3c1b174";

#[test]
fn takes_a_gzip_json_body_and_answers_in_json() {
    let directory = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(directory.path());
    let body = traces_request(
        &json!([{"key": "service.name", "value": {"stringValue": "shop"}}]),
        &json!([span_json("GET /cart", TRACE_ID_HEX, SPAN_ID_HEX)]),
    );
    let mut gzip_encoder = GzEncoder::new(Vec::new(), Compression::default());
    gzip_encoder
        .write_all(&serde_json::to_vec(&body).unwrap())
        .unwrap();
    let (status, response) = post_body(
        &receiver,
        "/v1/traces",
        &[
            ("Content-Type", "application/json; charset=utf-8"),
            ("Content-Encoding", "gzip"),
        ],
        &gzip_encoder.finish().unwrap(),
    );
    assert_eq!((status, response), (200, json!({"partialSuccess": null})));
    receiver.stop_and_wait_for_writer();
    let names: Vec<String> = query_first_column(
        &open_telemetry_file(directory.path()),
        "SELECT name FROM spans",
    );
    assert_eq!(names, ["GET /cart"]);
}

#[test]
fn refuses_another_content_type() {
    let directory = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(directory.path());
    let (status, _) = post_body(
        &receiver,
        "/v1/logs",
        &[("Content-Type", "text/plain")],
        b"hi",
    );
    assert_eq!(status, 415);
    let (status, response) = post_body(
        &receiver,
        "/v1/logs",
        &[("Content-Type", "application/json")],
        b"{\"resourceLogs\": 7}",
    );
    assert_eq!(status, 400);
    assert_eq!(response["code"], 3, "{response}");
    receiver.stop_and_wait_for_writer();
}

#[test]
fn rejects_a_span_without_ids_and_keeps_the_rest() {
    let directory = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(directory.path());
    let body = traces_request(
        &json!([]),
        &json!([
            span_json("GET /cart", TRACE_ID_HEX, SPAN_ID_HEX),
            span_json("broken", "00000000000000000000000000000000", SPAN_ID_HEX),
        ]),
    );
    let (status, response) = post_json(&receiver, "/v1/traces", &body);
    assert_eq!(status, 200);
    assert_eq!(response["partialSuccess"]["rejectedSpans"], 1, "{response}");
    receiver.stop_and_wait_for_writer();
    let spans: Vec<String> = query_first_column(
        &open_telemetry_file(directory.path()),
        "SELECT resource.service || ' ' || span.name
         FROM spans span
         JOIN resources resource ON resource.id = span.resource_id",
    );
    assert_eq!(spans, ["unknown_service GET /cart"]);
}

#[test]
fn rejects_the_metric_types_the_store_lacks() {
    let directory = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(directory.path());
    let recorded_at = now_unix_nanos().to_string();
    let body = json!({"resourceMetrics": [{"scopeMetrics": [{"metrics": [
        {"name": "queue.depth", "gauge": {"dataPoints": [{"timeUnixNano": recorded_at, "asInt": "4"}]}},
        {"name": "latency", "summary": {"dataPoints": [{"timeUnixNano": recorded_at}, {"timeUnixNano": recorded_at}]}},
    ]}]}]});
    let (status, response) = post_json(&receiver, "/v1/metrics", &body);
    assert_eq!(status, 200);
    assert_eq!(
        response["partialSuccess"],
        json!({"rejectedDataPoints": 2, "errorMessage": "otelo does not store summaries"})
    );
    receiver.stop_and_wait_for_writer();
    let points: Vec<String> = query_first_column(
        &open_telemetry_file(directory.path()),
        "SELECT metric_series.name || ' ' || metric_point.value
         FROM metric_points metric_point
         JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         WHERE metric_series.name NOT LIKE 'otelo.%'",
    );
    assert_eq!(points, ["queue.depth 4.0"]);
}

#[test]
fn keeps_the_kind_of_a_sum_and_the_buckets_of_an_exponential_histogram() {
    let directory = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(directory.path());
    let recorded_at = now_unix_nanos().to_string();
    let sum_metric = |name: &str, is_monotonic: bool, temporality: Value| {
        let mut sum = json!({
            "isMonotonic": is_monotonic,
            "dataPoints": [{"timeUnixNano": recorded_at, "asInt": "7"}],
        });
        if !temporality.is_null() {
            sum["aggregationTemporality"] = temporality;
        }
        json!({"name": name, "sum": sum})
    };
    let body = json!({"resourceMetrics": [{"scopeMetrics": [{"metrics": [
        sum_metric("bytes.sent", true, json!(1)),
        sum_metric("emails.sent", true, json!(2)),
        sum_metric("requests.active", false, json!(2)),
        sum_metric("queue.drift", false, json!(1)),
        sum_metric("untold", true, Value::Null),
        {"name": "request.duration", "exponentialHistogram": {
            "aggregationTemporality": 2,
            "dataPoints": [
                {
                    "timeUnixNano": recorded_at, "count": "4", "sum": 7.0, "scale": 1, "zeroCount": "1",
                    "positive": {"offset": -2, "bucketCounts": ["1", "2"]},
                },
                // No value was recorded: the series ended.
                {"timeUnixNano": recorded_at, "flags": 1, "attributes": [
                    {"key": "route", "value": {"stringValue": "/gone"}},
                ]},
            ],
        }},
    ]}]}]});
    let (status, response) = post_json(&receiver, "/v1/metrics", &body);
    assert_eq!(status, 200);
    assert_eq!(
        response["partialSuccess"],
        json!({
            "rejectedDataPoints": 2,
            "errorMessage": "a sum needs an aggregation temporality; \
                             otelo does not store delta sums that are not monotonic",
        })
    );
    receiver.stop_and_wait_for_writer();
    let series: Vec<String> = query_first_column(
        &open_telemetry_file(directory.path()),
        "SELECT json_object('name', metric_series.name, 'kind', metric_series.kind,
                            'temporality', metric_series.aggregation_temporality, 'value', metric_point.value,
                            'scale', metric_point.histogram -> 'scale',
                            'zero', metric_point.histogram -> 'zero_count',
                            'positive', metric_point.histogram -> 'positive',
                            'negative', metric_point.histogram -> 'negative')
         FROM metric_points metric_point
         JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         WHERE metric_series.name NOT LIKE 'otelo.%'
         ORDER BY metric_series.name",
    );
    assert_eq!(
        series,
        [
            r#"{"name":"bytes.sent","kind":"counter","temporality":"delta","value":7.0,"scale":null,"zero":null,"positive":null,"negative":null}"#,
            r#"{"name":"emails.sent","kind":"counter","temporality":"cumulative","value":7.0,"scale":null,"zero":null,"positive":null,"negative":null}"#,
            r#"{"name":"request.duration","kind":"histogram","temporality":"cumulative","value":7.0,"scale":1,"zero":1,"positive":{"offset":-2,"counts":[1,2]},"negative":{"offset":0,"counts":[]}}"#,
            r#"{"name":"requests.active","kind":"updown","temporality":null,"value":7.0,"scale":null,"zero":null,"positive":null,"negative":null}"#,
        ]
    );
}

#[test]
fn a_full_queue_rejects_the_whole_request() {
    let (receiver, _inbox) = Receiver::start_with_full_queue();
    let body = traces_request(
        &json!([]),
        &json!([
            span_json("a", TRACE_ID_HEX, SPAN_ID_HEX),
            span_json("b", TRACE_ID_HEX, SPAN_ID_HEX)
        ]),
    );
    let (_, first_response) = post_json(&receiver, "/v1/traces", &body);
    assert_eq!(first_response["partialSuccess"], Value::Null);
    let (status, second_response) = post_json(&receiver, "/v1/traces", &body);
    assert_eq!(status, 200);
    assert_eq!(
        second_response["partialSuccess"]["rejectedSpans"], 2,
        "{second_response}"
    );
    receiver.stop_and_wait_for_writer();
}

const fn any_value(value: any_value::Value) -> AnyValue {
    AnyValue { value: Some(value) }
}

#[test]
fn json_and_protobuf_of_one_request_give_the_same_frame() {
    let directory = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(directory.path());
    let logged_at = now_unix_nanos();
    let json_body = json!({"resourceLogs": [{"scopeLogs": [{"logRecords": [{
        "timeUnixNano": logged_at.to_string(),
        "body": {"stringValue": "cart is empty"},
        "attributes": [{"key": "cart.items", "value": {"intValue": "0"}}],
        "traceId": TRACE_ID_HEX,
        "spanId": SPAN_ID_HEX,
    }]}]}]});
    let request = ExportLogsServiceRequest {
        resource_logs: vec![ResourceLogs {
            scope_logs: vec![ScopeLogs {
                log_records: vec![LogRecord {
                    time_unix_nano: u64::try_from(logged_at).unwrap(),
                    body: Some(any_value(any_value::Value::StringValue(
                        "cart is empty".into(),
                    ))),
                    attributes: vec![KeyValue {
                        key: "cart.items".into(),
                        value: Some(any_value(any_value::Value::IntValue(0))),
                        ..KeyValue::default()
                    }],
                    trace_id: hex_to_bytes(TRACE_ID_HEX),
                    span_id: hex_to_bytes(SPAN_ID_HEX),
                    ..LogRecord::default()
                }],
                ..ScopeLogs::default()
            }],
            ..ResourceLogs::default()
        }],
    };

    let (json_status, _) = post_json(&receiver, "/v1/logs", &json_body);
    let (protobuf_status, _) = post_body(
        &receiver,
        "/v1/logs",
        &[("Content-Type", "application/x-protobuf")],
        &request.encode_to_vec(),
    );
    let frames: Vec<_> = receiver
        .journal
        .read_frames(Signal::Logs, None)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    receiver.stop_and_wait_for_writer();

    assert_eq!((json_status, protobuf_status), (200, 200));
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].request, frames[1].request);
    assert_eq!(
        ExportLogsServiceRequest::decode(frames[0].request.as_slice()).unwrap(),
        request
    );
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|start| u8::from_str_radix(&hex[start..start + 2], 16).unwrap())
        .collect()
}

struct FailingJournal;

impl Journal for FailingJournal {
    fn append_frame(&self, _: Signal, _: i64, _: &[u8]) -> anyhow::Result<SyncTicket> {
        Err(anyhow::anyhow!("the disk is full"))
    }

    fn read_frames(&self, _: Signal, _: Option<Position>) -> anyhow::Result<Frames> {
        Ok(Box::new(std::iter::empty()))
    }

    fn size_in_bytes(&self) -> anyhow::Result<u64> {
        Ok(0)
    }
}

#[test]
fn a_request_the_journal_cannot_keep_is_unavailable_and_stays_out_of_the_store() {
    let (receiver, inbox) = Receiver::start_with_journal(TestJournal::of(Arc::new(FailingJournal)));
    let body = traces_request(
        &json!([]),
        &json!([span_json("a", TRACE_ID_HEX, SPAN_ID_HEX)]),
    );

    let (status, response) = post_json(&receiver, "/v1/traces", &body);
    receiver.stop_and_wait_for_writer();

    assert_eq!(status, 503);
    assert_eq!(response["code"], 14, "{response}");
    assert!(
        response["message"]
            .as_str()
            .unwrap()
            .contains("the disk is full"),
        "{response}"
    );
    assert_eq!(inbox.take_queued_batches().count(), 0);
}
