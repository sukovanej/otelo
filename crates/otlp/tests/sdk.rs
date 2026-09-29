mod common;

use std::time::SystemTime;

use common::{Receiver, open_todays_day_file, rows};
use opentelemetry::logs::{LogRecord, Logger, LoggerProvider, Severity};
use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::{Span, SpanKind, Status, TraceContextExt, Tracer, TracerProvider};
use opentelemetry::{Context, KeyValue};
use opentelemetry_otlp::{
    Compression, LogExporter, MetricExporter, Protocol, SpanExporter, WithExportConfig,
    WithTonicConfig,
};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;

#[derive(Clone, Copy)]
enum Transport {
    Grpc,
    HttpProtobuf,
    HttpJson,
}

#[test]
fn grpc_with_gzip_takes_each_signal() {
    round_trip(Transport::Grpc);
}

#[test]
fn http_protobuf_takes_each_signal() {
    round_trip(Transport::HttpProtobuf);
}

#[test]
fn http_json_takes_each_signal() {
    round_trip(Transport::HttpJson);
}

fn round_trip(transport: Transport) {
    let dir = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(dir.path());
    send_each_signal(&receiver, transport);
    receiver.stop_and_wait_for_writer();
    let conn = open_todays_day_file(dir.path());

    let services: Vec<String> = rows(&conn, "SELECT service FROM resources ORDER BY id");
    assert_eq!(services, ["shop", "otelo"]);
    let host: Vec<String> = rows(
        &conn,
        "SELECT attributes ->> '$.\"host.name\"' FROM resources WHERE service = 'shop'",
    );
    assert_eq!(host, ["droplet"]);

    let spans: Vec<String> = rows(
        &conn,
        "SELECT json_object(
             'name', name, 'kind', kind, 'status', status, 'root', parent_span_id IS NULL,
             'route', attributes ->> '$.\"http.route\"',
             'scope', attributes ->> '$.\"otel.scope.name\"',
             'description', attributes ->> '$.\"otel.status_description\"')
         FROM spans ORDER BY start_ts",
    );
    assert_eq!(
        spans,
        [
            r#"{"name":"GET /cart","kind":2,"status":0,"root":1,"route":"/cart","scope":"shop-test","description":null}"#,
            r#"{"name":"SELECT cart","kind":1,"status":2,"root":0,"route":null,"scope":"shop-test","description":"timeout"}"#,
        ]
    );
    let events: Vec<String> = rows(
        &conn,
        "SELECT json_object('name', e.value ->> 'name', 'attempt', e.value ->> '$.attributes.attempt')
         FROM spans, json_each(spans.events) e",
    );
    assert_eq!(events, [r#"{"name":"retry","attempt":2}"#]);
    let children: Vec<i64> = rows(
        &conn,
        "SELECT count(*) FROM spans c
         JOIN spans p ON p.trace_id = c.trace_id AND p.span_id = c.parent_span_id",
    );
    assert_eq!(children, [1]);

    let logs: Vec<String> = rows(
        &conn,
        "SELECT json_object('body', body, 'severity', severity, 'source', source,
                            'user', attributes ->> '$.\"user.id\"',
                            'scope', attributes ->> '$.\"otel.scope.name\"')
         FROM logs",
    );
    assert_eq!(
        logs,
        [r#"{"body":"cart is empty","severity":13,"source":"otlp","user":7,"scope":"shop-test"}"#]
    );
    let in_span: Vec<String> = rows(
        &conn,
        "SELECT s.name FROM logs l JOIN spans s ON s.trace_id = l.trace_id AND s.span_id = l.span_id",
    );
    assert_eq!(in_span, ["GET /cart"]);

    let points: Vec<String> = rows(
        &conn,
        "SELECT json_object('name', s.name, 'kind', s.kind, 'unit', s.unit,
                            'plan', s.labels ->> 'plan', 'value', p.value,
                            'counts', p.histogram -> 'counts', 'bounds', p.histogram -> 'bounds',
                            'cumulative', p.histogram -> 'cumulative')
         FROM points p
         JOIN series s ON s.id = p.series_id
         JOIN resources r ON r.id = s.resource_id
         WHERE r.service = 'shop' ORDER BY s.name",
    );
    assert_eq!(
        points,
        [
            r#"{"name":"cart.adds","kind":"sum","unit":"{item}","plan":"free","value":3.0,"counts":null,"bounds":null,"cumulative":null}"#,
            r#"{"name":"cart.duration","kind":"histogram","unit":"ms","plan":"free","value":55.0,"counts":[1,1,0],"bounds":[10.0,100.0],"cumulative":true}"#,
        ]
    );
}

fn send_each_signal(receiver: &Receiver, transport: Transport) {
    let (spans, logs, metrics) = exporters(receiver, transport);
    let resource = Resource::builder()
        .with_service_name("shop")
        .with_attribute(KeyValue::new("host.name", "droplet"))
        .build();
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(spans)
        .with_resource(resource.clone())
        .build();
    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(logs)
        .with_resource(resource.clone())
        .build();
    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(metrics)
        .with_resource(resource)
        .build();

    let tracer = tracer_provider.tracer("shop-test");
    let root = tracer
        .span_builder("GET /cart")
        .with_kind(SpanKind::Server)
        .with_attributes([KeyValue::new("http.route", "/cart")])
        .start(&tracer);
    let cx = Context::current_with_span(root);
    let mut child = tracer.start_with_context("SELECT cart", &cx);
    child.add_event("retry", vec![KeyValue::new("attempt", 2)]);
    child.set_status(Status::error("timeout"));
    child.end();

    let logger = logger_provider.logger("shop-test");
    let mut record = logger.create_log_record();
    record.set_body("cart is empty".into());
    record.set_severity_number(Severity::Warn);
    record.set_timestamp(SystemTime::now());
    record.add_attribute("user.id", 7);
    let context = cx.span().span_context().clone();
    record.set_trace_context(context.trace_id(), context.span_id(), None);
    logger.emit(record);
    cx.span().end();

    let meter = meter_provider.meter("shop-test");
    let plan = [KeyValue::new("plan", "free")];
    let adds = meter.u64_counter("cart.adds").with_unit("{item}").build();
    adds.add(3, &plan);
    let duration = meter
        .f64_histogram("cart.duration")
        .with_unit("ms")
        .with_boundaries(vec![10.0, 100.0])
        .build();
    duration.record(5.0, &plan);
    duration.record(50.0, &plan);

    // Shutting a provider down exports what it holds.
    tracer_provider.shutdown().unwrap();
    logger_provider.shutdown().unwrap();
    meter_provider.shutdown().unwrap();
}

fn exporters(
    receiver: &Receiver,
    transport: Transport,
) -> (SpanExporter, LogExporter, MetricExporter) {
    match transport {
        Transport::Grpc => {
            // The tonic client starts its connection on the runtime.
            let _runtime = receiver.runtime.enter();
            let endpoint = format!("http://{}", receiver.grpc);
            (
                SpanExporter::builder()
                    .with_tonic()
                    .with_endpoint(&endpoint)
                    .with_compression(Compression::Gzip)
                    .build()
                    .unwrap(),
                LogExporter::builder()
                    .with_tonic()
                    .with_endpoint(&endpoint)
                    .with_compression(Compression::Gzip)
                    .build()
                    .unwrap(),
                MetricExporter::builder()
                    .with_tonic()
                    .with_endpoint(&endpoint)
                    .with_compression(Compression::Gzip)
                    .build()
                    .unwrap(),
            )
        }
        Transport::HttpProtobuf | Transport::HttpJson => {
            let protocol = match transport {
                Transport::HttpJson => Protocol::HttpJson,
                _ => Protocol::HttpBinary,
            };
            (
                SpanExporter::builder()
                    .with_http()
                    .with_protocol(protocol)
                    .with_endpoint(receiver.url("/v1/traces"))
                    .build()
                    .unwrap(),
                LogExporter::builder()
                    .with_http()
                    .with_protocol(protocol)
                    .with_endpoint(receiver.url("/v1/logs"))
                    .build()
                    .unwrap(),
                MetricExporter::builder()
                    .with_http()
                    .with_protocol(protocol)
                    .with_endpoint(receiver.url("/v1/metrics"))
                    .build()
                    .unwrap(),
            )
        }
    }
}
