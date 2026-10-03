mod common;

use std::time::SystemTime;

use common::{Receiver, open_telemetry_file, query_first_column};
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
    send_each_signal_and_check_the_telemetry_file(Transport::Grpc);
}

#[test]
fn http_protobuf_takes_each_signal() {
    send_each_signal_and_check_the_telemetry_file(Transport::HttpProtobuf);
}

#[test]
fn http_json_takes_each_signal() {
    send_each_signal_and_check_the_telemetry_file(Transport::HttpJson);
}

fn send_each_signal_and_check_the_telemetry_file(transport: Transport) {
    let directory = tempfile::tempdir().unwrap();
    let receiver = Receiver::start_writing_into(directory.path());
    send_each_signal(&receiver, transport);
    receiver.stop_and_wait_for_writer();
    let connection = open_telemetry_file(directory.path());

    let services: Vec<String> =
        query_first_column(&connection, "SELECT service FROM resources ORDER BY id");
    assert_eq!(services, ["shop", "otelo"]);
    let host_names: Vec<String> = query_first_column(
        &connection,
        "SELECT attributes ->> '$.\"host.name\"' FROM resources WHERE service = 'shop'",
    );
    assert_eq!(host_names, ["droplet"]);

    let spans: Vec<String> = query_first_column(
        &connection,
        "SELECT json_object(
             'name', name, 'kind', kind, 'status', status_code, 'root', parent_span_id IS NULL,
             'route', attributes ->> '$.\"http.route\"',
             'scope', attributes ->> '$.\"otel.scope.name\"',
             'description', attributes ->> '$.\"otel.status_description\"')
         FROM spans
         ORDER BY started_at",
    );
    assert_eq!(
        spans,
        [
            r#"{"name":"GET /cart","kind":2,"status":0,"root":1,"route":"/cart","scope":"shop-test","description":null}"#,
            r#"{"name":"SELECT cart","kind":1,"status":2,"root":0,"route":null,"scope":"shop-test","description":"timeout"}"#,
        ]
    );
    let events: Vec<String> = query_first_column(
        &connection,
        "SELECT json_object('name', event.value ->> 'name',
                            'attempt', event.value ->> '$.attributes.attempt')
         FROM spans, json_each(spans.events) event",
    );
    assert_eq!(events, [r#"{"name":"retry","attempt":2}"#]);
    let child_span_counts: Vec<i64> = query_first_column(
        &connection,
        "SELECT count(*)
         FROM spans child_span
         JOIN spans parent_span
           ON parent_span.trace_id = child_span.trace_id
          AND parent_span.span_id = child_span.parent_span_id",
    );
    assert_eq!(child_span_counts, [1]);

    let logs: Vec<String> = query_first_column(
        &connection,
        "SELECT json_object('body', body, 'severity', severity_number,
                            'user', attributes ->> '$.\"user.id\"',
                            'scope', attributes ->> '$.\"otel.scope.name\"')
         FROM logs",
    );
    assert_eq!(
        logs,
        [r#"{"body":"cart is empty","severity":13,"user":7,"scope":"shop-test"}"#]
    );
    let spans_of_the_log: Vec<String> = query_first_column(
        &connection,
        "SELECT span.name
         FROM logs log
         JOIN spans span ON span.trace_id = log.trace_id AND span.span_id = log.span_id",
    );
    assert_eq!(spans_of_the_log, ["GET /cart"]);

    let points: Vec<String> = query_first_column(
        &connection,
        "SELECT json_object('name', metric_series.name, 'kind', metric_series.kind,
                            'temporality', metric_series.aggregation_temporality,
                            'unit', metric_series.unit,
                            'plan', metric_series.attributes ->> 'plan', 'value', point.value,
                            'counts', point.histogram -> 'counts',
                            'bounds', point.histogram -> 'bounds')
         FROM metric_points point
         JOIN metric_series ON metric_series.id = point.metric_series_id
         JOIN resources resource ON resource.id = metric_series.resource_id
         WHERE resource.service = 'shop'
         ORDER BY metric_series.name",
    );
    assert_eq!(
        points,
        [
            r#"{"name":"cart.adds","kind":"counter","temporality":"cumulative","unit":"{item}","plan":"free","value":3.0,"counts":null,"bounds":null}"#,
            r#"{"name":"cart.duration","kind":"histogram","temporality":"cumulative","unit":"ms","plan":"free","value":55.0,"counts":[1,1,0],"bounds":[10.0,100.0]}"#,
            r#"{"name":"cart.items","kind":"updown","temporality":null,"unit":"{item}","plan":"free","value":2.0,"counts":null,"bounds":null}"#,
        ]
    );
}

fn send_each_signal(receiver: &Receiver, transport: Transport) {
    let (span_exporter, log_exporter, metric_exporter) = build_exporters(receiver, transport);
    let resource = Resource::builder()
        .with_service_name("shop")
        .with_attribute(KeyValue::new("host.name", "droplet"))
        .build();
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_resource(resource.clone())
        .build();
    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(resource.clone())
        .build();
    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(metric_exporter)
        .with_resource(resource)
        .build();

    let tracer = tracer_provider.tracer("shop-test");
    let root = tracer
        .span_builder("GET /cart")
        .with_kind(SpanKind::Server)
        .with_attributes([KeyValue::new("http.route", "/cart")])
        .start(&tracer);
    let context = Context::current_with_span(root);
    let mut child = tracer.start_with_context("SELECT cart", &context);
    child.add_event("retry", vec![KeyValue::new("attempt", 2)]);
    child.set_status(Status::error("timeout"));
    child.end();

    let logger = logger_provider.logger("shop-test");
    let mut record = logger.create_log_record();
    record.set_body("cart is empty".into());
    record.set_severity_number(Severity::Warn);
    record.set_timestamp(SystemTime::now());
    record.add_attribute("user.id", 7);
    let span_context = context.span().span_context().clone();
    record.set_trace_context(span_context.trace_id(), span_context.span_id(), None);
    logger.emit(record);
    context.span().end();

    let meter = meter_provider.meter("shop-test");
    let plan_attributes = [KeyValue::new("plan", "free")];
    let adds_counter = meter.u64_counter("cart.adds").with_unit("{item}").build();
    adds_counter.add(3, &plan_attributes);
    let duration_histogram = meter
        .f64_histogram("cart.duration")
        .with_unit("ms")
        .with_boundaries(vec![10.0, 100.0])
        .build();
    duration_histogram.record(5.0, &plan_attributes);
    duration_histogram.record(50.0, &plan_attributes);
    let item_counter = meter
        .i64_up_down_counter("cart.items")
        .with_unit("{item}")
        .build();
    item_counter.add(3, &plan_attributes);
    item_counter.add(-1, &plan_attributes);

    // Shutting a provider down exports what it holds.
    tracer_provider.shutdown().unwrap();
    logger_provider.shutdown().unwrap();
    meter_provider.shutdown().unwrap();
}

fn build_exporters(
    receiver: &Receiver,
    transport: Transport,
) -> (SpanExporter, LogExporter, MetricExporter) {
    match transport {
        Transport::Grpc => {
            // The tonic client starts its connection on the runtime.
            let _runtime = receiver.runtime.enter();
            let endpoint = format!("http://{}", receiver.grpc_address);
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
                    .with_endpoint(receiver.http_url("/v1/traces"))
                    .build()
                    .unwrap(),
                LogExporter::builder()
                    .with_http()
                    .with_protocol(protocol)
                    .with_endpoint(receiver.http_url("/v1/logs"))
                    .build()
                    .unwrap(),
                MetricExporter::builder()
                    .with_http()
                    .with_protocol(protocol)
                    .with_endpoint(receiver.http_url("/v1/metrics"))
                    .build()
                    .unwrap(),
            )
        }
    }
}
