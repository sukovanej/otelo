//! The one mapping from OTLP to the rows of the store, which both transports
//! call.

use std::collections::BTreeSet;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use opentelemetry_proto::tonic::metrics::v1::{
    AggregationTemporality, DataPointFlags, HistogramDataPoint, NumberDataPoint, metric,
    number_data_point,
};
use opentelemetry_proto::tonic::resource::v1::Resource as OtlpResource;
use opentelemetry_proto::tonic::trace::v1::Span as OtlpSpan;
use serde_json::{Map, Value, json};
use siner_telemetry::{
    Batch, Histogram, Log, Metric, MetricKind, Point, Records, Resource, Span, now,
};

/// The service of a resource without `service.name`, as the OpenTelemetry SDKs name it.
const UNKNOWN_SERVICE: &str = "unknown_service";

/// The rows of one export request, and what the mapping rejected.
#[derive(Debug, Default)]
pub struct Mapped {
    pub batch: Batch,
    /// The log records, spans, or data points the request held.
    pub items: i64,
    pub rejected: i64,
    /// Why the rejected items were rejected, one reason each kind.
    pub reasons: BTreeSet<String>,
}

impl Mapped {
    /// Keeps the records of a resource that has any.
    fn push(&mut self, records: Records) {
        if !(records.logs.is_empty() && records.spans.is_empty() && records.metrics.is_empty()) {
            self.batch.push(records);
        }
    }

    fn reject(&mut self, count: usize, reason: impl Into<String>) {
        self.rejected += i64::try_from(count).unwrap_or(i64::MAX);
        self.reasons.insert(reason.into());
    }
}

#[must_use]
pub fn logs(request: ExportLogsServiceRequest) -> Mapped {
    let mut mapped = Mapped::default();
    for resource_logs in request.resource_logs {
        let mut records = records(resource_logs.resource);
        for scope_logs in resource_logs.scope_logs {
            let scope = scope(scope_logs.scope);
            for record in scope_logs.log_records {
                mapped.items += 1;
                records.logs.push(log(record, &scope));
            }
        }
        mapped.push(records);
    }
    mapped
}

#[must_use]
pub fn spans(request: ExportTraceServiceRequest) -> Mapped {
    let mut mapped = Mapped::default();
    for resource_spans in request.resource_spans {
        let mut records = records(resource_spans.resource);
        for scope_spans in resource_spans.scope_spans {
            let scope = scope(scope_spans.scope);
            for span in scope_spans.spans {
                mapped.items += 1;
                match self::span(span, &scope) {
                    Some(span) => records.spans.push(span),
                    None => mapped.reject(
                        1,
                        "a span needs a 16-byte trace ID and an 8-byte span ID, not all zeros",
                    ),
                }
            }
        }
        mapped.push(records);
    }
    mapped
}

#[must_use]
pub fn metrics(request: ExportMetricsServiceRequest) -> Mapped {
    let mut mapped = Mapped::default();
    for resource_metrics in request.resource_metrics {
        let mut records = records(resource_metrics.resource);
        for scope_metrics in resource_metrics.scope_metrics {
            let scope = scope(scope_metrics.scope);
            for otlp in scope_metrics.metrics {
                let series = Series {
                    name: otlp.name,
                    unit: otlp.unit,
                    scope: &scope,
                };
                match otlp.data {
                    Some(metric::Data::Gauge(gauge)) => {
                        numbers(
                            &mut mapped,
                            &mut records,
                            &series,
                            MetricKind::Gauge,
                            gauge.data_points,
                        );
                    }
                    Some(metric::Data::Sum(sum)) => {
                        numbers(
                            &mut mapped,
                            &mut records,
                            &series,
                            MetricKind::Sum,
                            sum.data_points,
                        );
                    }
                    Some(metric::Data::Histogram(histogram)) => {
                        let cumulative = histogram.aggregation_temporality
                            == AggregationTemporality::Cumulative as i32;
                        for point in histogram.data_points {
                            mapped.items += 1;
                            match series.histogram(point, cumulative) {
                                Ok(Some(metric)) => records.metrics.push(metric),
                                Ok(None) => {}
                                Err(error) => mapped.reject(1, format!("{error:#}")),
                            }
                        }
                    }
                    Some(metric::Data::ExponentialHistogram(histogram)) => {
                        let count = histogram.data_points.len();
                        mapped.items += i64::try_from(count).unwrap_or(i64::MAX);
                        mapped.reject(count, "siner does not store exponential histograms");
                    }
                    Some(metric::Data::Summary(summary)) => {
                        let count = summary.data_points.len();
                        mapped.items += i64::try_from(count).unwrap_or(i64::MAX);
                        mapped.reject(count, "siner does not store summaries");
                    }
                    None => {}
                }
            }
        }
        mapped.push(records);
    }
    mapped
}

fn records(resource: Option<OtlpResource>) -> Records {
    let attributes = attributes(resource.map(|r| r.attributes).unwrap_or_default());
    let service = match attributes.get("service.name") {
        Some(Value::String(name)) if !name.is_empty() => name.clone(),
        _ => UNKNOWN_SERVICE.into(),
    };
    Records {
        resource: Resource {
            service,
            attributes,
        },
        logs: Vec::new(),
        spans: Vec::new(),
        metrics: Vec::new(),
    }
}

/// The attributes that name the instrumentation scope, which each record of
/// the scope carries, as the OpenTelemetry spec maps a scope to formats without one.
fn scope(scope: Option<InstrumentationScope>) -> Map<String, Value> {
    let mut attributes = Map::new();
    if let Some(scope) = scope {
        if !scope.name.is_empty() {
            attributes.insert("otel.scope.name".into(), scope.name.into());
        }
        if !scope.version.is_empty() {
            attributes.insert("otel.scope.version".into(), scope.version.into());
        }
    }
    attributes
}

/// The scope attributes, then the record's own, which win a clash.
fn with_scope(scope: &Map<String, Value>, own: Vec<KeyValue>) -> Map<String, Value> {
    let mut attributes = scope.clone();
    attributes.extend(self::attributes(own));
    attributes
}

fn log(record: LogRecord, scope: &Map<String, Value>) -> Log {
    let ts = [record.time_unix_nano, record.observed_time_unix_nano]
        .into_iter()
        .find(|&ts| ts != 0)
        .map_or_else(now, nanos);
    let body = match record.body.and_then(|body| body.value) {
        Some(any_value::Value::StringValue(body)) => body,
        None => String::new(),
        Some(other) => value(other).to_string(),
    };
    let mut attributes = with_scope(scope, record.attributes);
    if !record.event_name.is_empty() {
        attributes.insert("event.name".into(), record.event_name.into());
    }
    Log {
        ts,
        severity: record.severity_number,
        body,
        trace_id: id(&record.trace_id),
        span_id: id(&record.span_id),
        attributes,
        source: "otlp",
    }
}

/// `None` when the span has no valid trace or span ID.
fn span(span: OtlpSpan, scope: &Map<String, Value>) -> Option<Span> {
    let mut attributes = with_scope(scope, span.attributes);
    let status = span.status.unwrap_or_default();
    if !status.message.is_empty() {
        attributes.insert("otel.status_description".into(), status.message.into());
    }
    let events = span
        .events
        .into_iter()
        .map(|event| {
            json!({
                "ts": nanos(event.time_unix_nano),
                "name": event.name,
                "attributes": self::attributes(event.attributes),
            })
        })
        .collect();
    Some(Span {
        trace_id: id(&span.trace_id)?,
        span_id: id(&span.span_id)?,
        parent_span_id: id(&span.parent_span_id),
        name: span.name,
        kind: span.kind,
        start_ts: nanos(span.start_time_unix_nano),
        duration_ns: nanos(
            span.end_time_unix_nano
                .saturating_sub(span.start_time_unix_nano),
        ),
        status: status.code,
        attributes,
        events,
    })
}

/// What the data points of one OTLP metric share.
struct Series<'a> {
    name: String,
    unit: String,
    scope: &'a Map<String, Value>,
}

impl Series<'_> {
    /// One series of one point. The writer finds the series of equal labels.
    fn metric(&self, kind: MetricKind, labels: Vec<KeyValue>, point: Point) -> Metric {
        Metric {
            name: self.name.clone(),
            kind,
            unit: self.unit.clone(),
            labels: with_scope(self.scope, labels),
            points: vec![point],
        }
    }

    /// `Ok(None)` for a point that says it has no value. A histogram point
    /// stores its sum as the value.
    fn histogram(
        &self,
        point: HistogramDataPoint,
        cumulative: bool,
    ) -> anyhow::Result<Option<Metric>> {
        if no_value(point.flags) {
            return Ok(None);
        }
        // A point without buckets has one bucket without bounds.
        let counts = if point.bucket_counts.is_empty() && point.explicit_bounds.is_empty() {
            vec![point.count]
        } else {
            point.bucket_counts
        };
        let histogram = Histogram {
            bounds: point.explicit_bounds,
            counts,
            count: point.count,
            sum: point.sum,
            min: point.min,
            max: point.max,
            cumulative,
        };
        histogram.check()?;
        let stored = Point {
            ts: nanos(point.time_unix_nano),
            value: histogram.sum.unwrap_or(0.0),
            histogram: Some(histogram),
        };
        Ok(Some(self.metric(
            MetricKind::Histogram,
            point.attributes,
            stored,
        )))
    }
}

fn numbers(
    mapped: &mut Mapped,
    records: &mut Records,
    series: &Series,
    kind: MetricKind,
    points: Vec<NumberDataPoint>,
) {
    for point in points {
        mapped.items += 1;
        if no_value(point.flags) {
            continue;
        }
        #[expect(clippy::cast_precision_loss, reason = "a store of f64 values")]
        let value = match point.value {
            Some(number_data_point::Value::AsDouble(value)) => value,
            Some(number_data_point::Value::AsInt(value)) => value as f64,
            None => {
                mapped.reject(1, "a number data point needs a value");
                continue;
            }
        };
        let stored = Point {
            ts: nanos(point.time_unix_nano),
            value,
            histogram: None,
        };
        records
            .metrics
            .push(series.metric(kind, point.attributes, stored));
    }
}

/// The point marks a gap in its series and holds no value.
const fn no_value(flags: u32) -> bool {
    flags & DataPointFlags::NoRecordedValueMask as u32 != 0
}

fn nanos(ts: u64) -> i64 {
    i64::try_from(ts).unwrap_or(i64::MAX)
}

/// A trace or span ID: `None` when it is empty, of the wrong length, or all
/// zeros, which OTLP counts as invalid.
fn id<const N: usize>(bytes: &[u8]) -> Option<[u8; N]> {
    let id: [u8; N] = bytes.try_into().ok()?;
    id.iter().any(|&byte| byte != 0).then_some(id)
}

fn attributes(list: Vec<KeyValue>) -> Map<String, Value> {
    list.into_iter()
        .map(|kv| {
            let value = kv.value.and_then(|v| v.value).map_or(Value::Null, value);
            (kv.key, value)
        })
        .collect()
}

/// An attribute value as JSON. Bytes are base64, as OTLP/JSON writes them.
fn value(value: any_value::Value) -> Value {
    match value {
        any_value::Value::StringValue(s) => Value::String(s),
        any_value::Value::BoolValue(b) => Value::Bool(b),
        any_value::Value::IntValue(i) => Value::from(i),
        any_value::Value::DoubleValue(d) => serde_json::Number::from_f64(d)
            .map_or_else(|| Value::String(d.to_string()), Value::Number),
        any_value::Value::ArrayValue(array) => Value::Array(
            array
                .values
                .into_iter()
                .map(|v: AnyValue| v.value.map_or(Value::Null, self::value))
                .collect(),
        ),
        any_value::Value::KvlistValue(list) => Value::Object(attributes(list.values)),
        any_value::Value::BytesValue(bytes) => Value::String(STANDARD.encode(bytes)),
        // Only profiles use the string table.
        any_value::Value::StringValueStrindex(_) => Value::Null,
    }
}
