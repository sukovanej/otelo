mod common;

use std::io::Write;

use common::{Receiver, open_todays_day_file, rows};
use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use siner_storage::now_unix_nanos;

fn post(receiver: &Receiver, path: &str, headers: &[(&str, &str)], body: &[u8]) -> (u16, Value) {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent.post(receiver.url(path));
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
    post(
        receiver,
        path,
        &[("Content-Type", "application/json")],
        &body,
    )
}

fn spans(resource: &Value, spans: &Value) -> Value {
    json!({"resourceSpans": [{
        "resource": {"attributes": resource},
        "scopeSpans": [{"spans": spans}],
    }]})
}

fn span(name: &str, trace_id: &str, span_id: &str) -> Value {
    let start = now_unix_nanos();
    json!({
        "traceId": trace_id,
        "spanId": span_id,
        "name": name,
        "kind": 2,
        "startTimeUnixNano": start.to_string(),
        "endTimeUnixNano": (start + 1_000_000).to_string(),
    })
}

const TRACE: &str = "5b8efff798038103d269b633813fc60c";
const SPAN: &str = "eee19b7ec3c1b174";

#[test]
fn takes_a_gzip_json_body_and_answers_in_json() {
    let dir = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(dir.path());
    let body = spans(
        &json!([{"key": "service.name", "value": {"stringValue": "shop"}}]),
        &json!([span("GET /cart", TRACE, SPAN)]),
    );
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(&serde_json::to_vec(&body).unwrap()).unwrap();
    let (status, response) = post(
        &receiver,
        "/v1/traces",
        &[
            ("Content-Type", "application/json; charset=utf-8"),
            ("Content-Encoding", "gzip"),
        ],
        &gzip.finish().unwrap(),
    );
    assert_eq!((status, response), (200, json!({"partialSuccess": null})));
    receiver.stop_and_wait_for_writer();
    let names: Vec<String> = rows(&open_todays_day_file(dir.path()), "SELECT name FROM spans");
    assert_eq!(names, ["GET /cart"]);
}

#[test]
fn refuses_another_content_type() {
    let dir = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(dir.path());
    let (status, _) = post(
        &receiver,
        "/v1/logs",
        &[("Content-Type", "text/plain")],
        b"hi",
    );
    assert_eq!(status, 415);
    let (status, response) = post(
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
    let dir = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(dir.path());
    let body = spans(
        &json!([]),
        &json!([
            span("GET /cart", TRACE, SPAN),
            span("broken", "00000000000000000000000000000000", SPAN),
        ]),
    );
    let (status, response) = post_json(&receiver, "/v1/traces", &body);
    assert_eq!(status, 200);
    assert_eq!(response["partialSuccess"]["rejectedSpans"], 1, "{response}");
    receiver.stop_and_wait_for_writer();
    let spans: Vec<String> = rows(
        &open_todays_day_file(dir.path()),
        "SELECT r.service || ' ' || s.name FROM spans s JOIN resources r ON r.id = s.resource_id",
    );
    assert_eq!(spans, ["unknown_service GET /cart"]);
}

#[test]
fn rejects_the_metric_types_the_store_lacks() {
    let dir = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(dir.path());
    let ts = now_unix_nanos().to_string();
    let body = json!({"resourceMetrics": [{"scopeMetrics": [{"metrics": [
        {"name": "queue.depth", "gauge": {"dataPoints": [{"timeUnixNano": ts, "asInt": "4"}]}},
        {"name": "latency", "summary": {"dataPoints": [{"timeUnixNano": ts}, {"timeUnixNano": ts}]}},
    ]}]}]});
    let (status, response) = post_json(&receiver, "/v1/metrics", &body);
    assert_eq!(status, 200);
    assert_eq!(
        response["partialSuccess"],
        json!({"rejectedDataPoints": 2, "errorMessage": "siner does not store summaries"})
    );
    receiver.stop_and_wait_for_writer();
    let points: Vec<String> = rows(
        &open_todays_day_file(dir.path()),
        "SELECT s.name || ' ' || p.value FROM points p JOIN series s ON s.id = p.series_id
         WHERE s.name != 'siner.telemetry.dropped_batches'",
    );
    assert_eq!(points, ["queue.depth 4.0"]);
}

#[test]
fn a_full_queue_rejects_the_whole_request() {
    let (receiver, _inbox) = Receiver::start_with_full_queue();
    let body = spans(
        &json!([]),
        &json!([span("a", TRACE, SPAN), span("b", TRACE, SPAN)]),
    );
    let (_, first) = post_json(&receiver, "/v1/traces", &body);
    assert_eq!(first["partialSuccess"], Value::Null);
    let (status, second) = post_json(&receiver, "/v1/traces", &body);
    assert_eq!(status, 200);
    assert_eq!(second["partialSuccess"]["rejectedSpans"], 2, "{second}");
    receiver.stop_and_wait_for_writer();
}
