use std::collections::BTreeSet;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use opentelemetry_proto::tonic::metrics::v1::{
    AggregationTemporality, DataPointFlags, ExponentialHistogramDataPoint, HistogramDataPoint,
    NumberDataPoint, exponential_histogram_data_point, metric, number_data_point,
};
use opentelemetry_proto::tonic::resource::v1::Resource as OtlpResource;
use opentelemetry_proto::tonic::trace::v1::Span as OtlpSpan;
use otelo_storage::{
    AttributeValue, Attributes, Batch, Buckets, ExplicitBuckets, ExponentialBuckets, Histogram,
    HistogramPoint, IndexedCounts, Log, Metric, NumberPoint, Points, Records, Resource, Severity,
    Span, SpanEvent, SpanId, SpanKind, SpanStatus, Temporality, TraceId, now_unix_nanos,
};

// The name the OpenTelemetry SDKs give a resource without `service.name`.
const UNKNOWN_SERVICE_NAME: &str = "unknown_service";

#[derive(Debug, Default)]
pub struct MappedExport {
    pub batch: Batch,
    pub item_count: i64,
    pub rejected_count: i64,
    pub rejection_reasons: BTreeSet<String>,
}

impl MappedExport {
    fn push_records_if_any(&mut self, records: Records) {
        if !(records.logs.is_empty() && records.spans.is_empty() && records.metrics.is_empty()) {
            self.batch.push(records);
        }
    }

    fn reject_items(&mut self, count: usize, reason: impl Into<String>) {
        self.rejected_count += i64::try_from(count).unwrap_or(i64::MAX);
        self.rejection_reasons.insert(reason.into());
    }

    fn reject_every_item(&mut self, count: usize, reason: impl Into<String>) {
        self.item_count += i64::try_from(count).unwrap_or(i64::MAX);
        self.reject_items(count, reason);
    }
}

#[must_use]
pub fn logs(request: ExportLogsServiceRequest) -> MappedExport {
    let mut mapped = MappedExport::default();
    for resource_logs in request.resource_logs {
        let mut records = records(resource_logs.resource);
        for scope_logs in resource_logs.scope_logs {
            let scope = scope_attributes(scope_logs.scope);
            for record in scope_logs.log_records {
                mapped.item_count += 1;
                records.logs.push(log(record, &scope));
            }
        }
        mapped.push_records_if_any(records);
    }
    mapped
}

#[must_use]
pub fn spans(request: ExportTraceServiceRequest) -> MappedExport {
    let mut mapped = MappedExport::default();
    for resource_spans in request.resource_spans {
        let mut records = records(resource_spans.resource);
        for scope_spans in resource_spans.scope_spans {
            let scope = scope_attributes(scope_spans.scope);
            for span in scope_spans.spans {
                mapped.item_count += 1;
                match valid_span(span, &scope) {
                    Some(span) => records.spans.push(span),
                    None => mapped.reject_items(
                        1,
                        "a span needs a 16-byte trace ID and an 8-byte span ID, not all zeros",
                    ),
                }
            }
        }
        mapped.push_records_if_any(records);
    }
    mapped
}

#[must_use]
pub fn metrics(request: ExportMetricsServiceRequest) -> MappedExport {
    let mut mapped = MappedExport::default();
    for resource_metrics in request.resource_metrics {
        let mut records = records(resource_metrics.resource);
        for scope_metrics in resource_metrics.scope_metrics {
            let scope = scope_attributes(scope_metrics.scope);
            for otlp in scope_metrics.metrics {
                let descriptor = MetricDescriptor {
                    name: otlp.name,
                    unit: otlp.unit,
                    scope: &scope,
                };
                match otlp.data {
                    Some(metric::Data::Gauge(gauge)) => {
                        numbers(
                            &mut mapped,
                            &mut records,
                            &descriptor,
                            NumberKind::Gauge,
                            gauge.data_points,
                        );
                    }
                    Some(metric::Data::Sum(sum)) => {
                        match kind_of_sum(sum.is_monotonic, sum.aggregation_temporality) {
                            Ok(kind) => {
                                numbers(
                                    &mut mapped,
                                    &mut records,
                                    &descriptor,
                                    kind,
                                    sum.data_points,
                                );
                            }
                            Err(reason) => mapped.reject_every_item(sum.data_points.len(), reason),
                        }
                    }
                    Some(metric::Data::Histogram(histogram)) => histograms(
                        &mut mapped,
                        &mut records,
                        &descriptor,
                        histogram.aggregation_temporality,
                        histogram
                            .data_points
                            .into_iter()
                            .map(explicit_histogram_point)
                            .collect(),
                    ),
                    Some(metric::Data::ExponentialHistogram(histogram)) => histograms(
                        &mut mapped,
                        &mut records,
                        &descriptor,
                        histogram.aggregation_temporality,
                        histogram
                            .data_points
                            .into_iter()
                            .map(exponential_histogram_point)
                            .collect(),
                    ),
                    Some(metric::Data::Summary(summary)) => {
                        mapped.reject_every_item(
                            summary.data_points.len(),
                            "otelo does not store summaries",
                        );
                    }
                    None => {}
                }
            }
        }
        mapped.push_records_if_any(records);
    }
    mapped
}

fn records(resource: Option<OtlpResource>) -> Records {
    let attributes = attributes(resource.map(|r| r.attributes).unwrap_or_default());
    let service = match attributes.get("service.name") {
        Some(AttributeValue::String(name)) if !name.is_empty() => name.clone(),
        _ => UNKNOWN_SERVICE_NAME.into(),
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

// The OpenTelemetry spec maps a scope to formats without one as attributes on each record.
fn scope_attributes(scope: Option<InstrumentationScope>) -> Attributes {
    let mut attributes = Attributes::new();
    if let Some(scope) = scope {
        if !scope.name.is_empty() {
            attributes.insert("otel.scope.name", scope.name);
        }
        if !scope.version.is_empty() {
            attributes.insert("otel.scope.version", scope.version);
        }
    }
    attributes
}

fn overlay_on_scope_attributes(scope: &Attributes, own: Vec<KeyValue>) -> Attributes {
    let mut attributes = scope.clone();
    attributes.extend(self::attributes(own));
    attributes
}

fn log(record: LogRecord, scope: &Attributes) -> Log {
    let logged_at = [record.time_unix_nano, record.observed_time_unix_nano]
        .into_iter()
        .find(|&unix_nanos| unix_nanos != 0)
        .map_or_else(now_unix_nanos, signed_nanos);
    let body = match record.body.and_then(|body| body.value) {
        Some(any_value::Value::StringValue(body)) => body,
        None => String::new(),
        Some(other) => attribute_value(other).to_string(),
    };
    let mut attributes = overlay_on_scope_attributes(scope, record.attributes);
    if !record.event_name.is_empty() {
        attributes.insert("event.name", record.event_name);
    }
    Log {
        logged_at,
        severity: Severity::from_number(record.severity_number),
        body,
        trace_id: valid_id_bytes(&record.trace_id).map(TraceId),
        span_id: valid_id_bytes(&record.span_id).map(SpanId),
        attributes,
        source: "otlp",
    }
}

fn valid_span(span: OtlpSpan, scope: &Attributes) -> Option<Span> {
    let mut attributes = overlay_on_scope_attributes(scope, span.attributes);
    let status = span.status.unwrap_or_default();
    if !status.message.is_empty() {
        attributes.insert("otel.status_description", status.message);
    }
    let events = span
        .events
        .into_iter()
        .map(|event| SpanEvent {
            occurred_at: signed_nanos(event.time_unix_nano),
            name: event.name,
            attributes: self::attributes(event.attributes),
        })
        .collect();
    Some(Span {
        trace_id: TraceId(valid_id_bytes(&span.trace_id)?),
        span_id: SpanId(valid_id_bytes(&span.span_id)?),
        parent_span_id: valid_id_bytes(&span.parent_span_id).map(SpanId),
        name: span.name,
        kind: SpanKind::from_number(span.kind),
        started_at: signed_nanos(span.start_time_unix_nano),
        duration_ns: signed_nanos(
            span.end_time_unix_nano
                .saturating_sub(span.start_time_unix_nano),
        ),
        status: SpanStatus::from_number(status.code),
        attributes,
        events,
    })
}

struct MetricDescriptor<'a> {
    name: String,
    unit: String,
    scope: &'a Attributes,
}

impl MetricDescriptor<'_> {
    // The writer merges the metrics of equal labels into one series.
    fn metric_of_points(&self, labels: Vec<KeyValue>, points: Points) -> Metric {
        Metric {
            name: self.name.clone(),
            unit: self.unit.clone(),
            labels: overlay_on_scope_attributes(self.scope, labels),
            points,
        }
    }

    fn histogram_metric(
        &self,
        temporality: Temporality,
        point: MappedHistogramPoint,
    ) -> anyhow::Result<Metric> {
        point.histogram.check_buckets()?;
        let stored = HistogramPoint {
            recorded_at: signed_nanos(point.time_unix_nano),
            histogram: point.histogram,
        };
        Ok(self.metric_of_points(point.labels, Points::Histogram(temporality, vec![stored])))
    }
}

#[derive(Clone, Copy)]
enum NumberKind {
    Gauge,
    UpDown,
    Counter(Temporality),
}

impl NumberKind {
    const fn wrap_points(self, points: Vec<NumberPoint>) -> Points {
        match self {
            Self::Gauge => Points::Gauge(points),
            Self::UpDown => Points::UpDown(points),
            Self::Counter(temporality) => Points::Counter(temporality, points),
        }
    }
}

fn temporality_of(aggregation_temporality: i32) -> Option<Temporality> {
    match AggregationTemporality::try_from(aggregation_temporality) {
        Ok(AggregationTemporality::Cumulative) => Some(Temporality::Cumulative),
        Ok(AggregationTemporality::Delta) => Some(Temporality::Delta),
        Ok(AggregationTemporality::Unspecified) | Err(_) => None,
    }
}

fn kind_of_sum(
    is_monotonic: bool,
    aggregation_temporality: i32,
) -> Result<NumberKind, &'static str> {
    match (is_monotonic, temporality_of(aggregation_temporality)) {
        (_, None) => Err("a sum needs an aggregation temporality"),
        (true, Some(temporality)) => Ok(NumberKind::Counter(temporality)),
        (false, Some(Temporality::Cumulative)) => Ok(NumberKind::UpDown),
        // Its level is a running total with no known start.
        (false, Some(Temporality::Delta)) => {
            Err("otelo does not store delta sums that are not monotonic")
        }
    }
}

struct MappedHistogramPoint {
    flags: u32,
    labels: Vec<KeyValue>,
    time_unix_nano: u64,
    histogram: Histogram,
}

fn explicit_histogram_point(point: HistogramDataPoint) -> MappedHistogramPoint {
    // The store needs one more count than bounds, and OTLP allows a point without buckets.
    let counts = if point.bucket_counts.is_empty() && point.explicit_bounds.is_empty() {
        vec![point.count]
    } else {
        point.bucket_counts
    };
    MappedHistogramPoint {
        flags: point.flags,
        labels: point.attributes,
        time_unix_nano: point.time_unix_nano,
        histogram: Histogram {
            count: point.count,
            sum: point.sum,
            min: point.min,
            max: point.max,
            buckets: Buckets::Explicit(ExplicitBuckets {
                bounds: point.explicit_bounds,
                counts,
            }),
        },
    }
}

fn exponential_histogram_point(point: ExponentialHistogramDataPoint) -> MappedHistogramPoint {
    MappedHistogramPoint {
        flags: point.flags,
        labels: point.attributes,
        time_unix_nano: point.time_unix_nano,
        histogram: Histogram {
            count: point.count,
            sum: point.sum,
            min: point.min,
            max: point.max,
            buckets: Buckets::Exponential(ExponentialBuckets {
                scale: point.scale,
                zero_count: point.zero_count,
                positive: indexed_counts(point.positive),
                negative: indexed_counts(point.negative),
            }),
        },
    }
}

fn indexed_counts(buckets: Option<exponential_histogram_data_point::Buckets>) -> IndexedCounts {
    buckets.map_or_else(IndexedCounts::default, |buckets| IndexedCounts {
        offset: buckets.offset,
        counts: buckets.bucket_counts,
    })
}

fn histograms(
    mapped: &mut MappedExport,
    records: &mut Records,
    descriptor: &MetricDescriptor,
    aggregation_temporality: i32,
    points: Vec<MappedHistogramPoint>,
) {
    let Some(temporality) = temporality_of(aggregation_temporality) else {
        mapped.reject_every_item(points.len(), "a histogram needs an aggregation temporality");
        return;
    };
    for point in points {
        mapped.item_count += 1;
        if has_no_recorded_value(point.flags) {
            continue;
        }
        match descriptor.histogram_metric(temporality, point) {
            Ok(metric) => records.metrics.push(metric),
            Err(error) => mapped.reject_items(1, format!("{error:#}")),
        }
    }
}

fn numbers(
    mapped: &mut MappedExport,
    records: &mut Records,
    descriptor: &MetricDescriptor,
    kind: NumberKind,
    points: Vec<NumberDataPoint>,
) {
    for point in points {
        mapped.item_count += 1;
        if has_no_recorded_value(point.flags) {
            continue;
        }
        #[expect(clippy::cast_precision_loss, reason = "a store of f64 values")]
        let value = match point.value {
            Some(number_data_point::Value::AsDouble(value)) => value,
            Some(number_data_point::Value::AsInt(value)) => value as f64,
            None => {
                mapped.reject_items(1, "a number data point needs a value");
                continue;
            }
        };
        let stored = NumberPoint {
            recorded_at: signed_nanos(point.time_unix_nano),
            value,
        };
        records
            .metrics
            .push(descriptor.metric_of_points(point.attributes, kind.wrap_points(vec![stored])));
    }
}

const fn has_no_recorded_value(flags: u32) -> bool {
    flags & DataPointFlags::NoRecordedValueMask as u32 != 0
}

fn signed_nanos(unix_nanos: u64) -> i64 {
    i64::try_from(unix_nanos).unwrap_or(i64::MAX)
}

// OTLP counts an all-zero trace or span ID as invalid.
fn valid_id_bytes<const N: usize>(bytes: &[u8]) -> Option<[u8; N]> {
    let id: [u8; N] = bytes.try_into().ok()?;
    id.iter().any(|&byte| byte != 0).then_some(id)
}

fn attributes(list: Vec<KeyValue>) -> Attributes {
    list.into_iter()
        .map(|kv| {
            let value = kv
                .value
                .and_then(|v| v.value)
                .map_or(AttributeValue::Null, attribute_value);
            (kv.key, value)
        })
        .collect()
}

fn attribute_value(value: any_value::Value) -> AttributeValue {
    match value {
        any_value::Value::StringValue(s) => AttributeValue::String(s),
        any_value::Value::BoolValue(b) => AttributeValue::Bool(b),
        any_value::Value::IntValue(i) => AttributeValue::Int(i),
        any_value::Value::DoubleValue(d) if d.is_finite() => AttributeValue::Double(d),
        // JSON cannot hold NaN or an infinity.
        any_value::Value::DoubleValue(d) => AttributeValue::String(d.to_string()),
        any_value::Value::ArrayValue(array) => AttributeValue::Array(
            array
                .values
                .into_iter()
                .map(|v: AnyValue| v.value.map_or(AttributeValue::Null, attribute_value))
                .collect(),
        ),
        any_value::Value::KvlistValue(list) => AttributeValue::Map(attributes(list.values)),
        // OTLP/JSON writes bytes as base64.
        any_value::Value::BytesValue(bytes) => AttributeValue::String(STANDARD.encode(bytes)),
        // Only profiles use the string table.
        any_value::Value::StringValueStrindex(_) => AttributeValue::Null,
    }
}
