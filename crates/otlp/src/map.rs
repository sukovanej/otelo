use std::collections::BTreeMap;

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
use otelo_indexed_storage::{
    AttributeValue, Attributes, Batch, Buckets, ExplicitBuckets, ExponentialBuckets, Histogram,
    HistogramPoint, IndexedCounts, Log, Metric, NumberPoint, Points, Records, Resource, Severity,
    Span, SpanEvent, SpanId, SpanKind, SpanStatus, Temporality, TraceContext, TraceId,
    now_unix_nanos,
};

// The name the OpenTelemetry SDKs give a resource without `service.name`.
const UNKNOWN_SERVICE_NAME: &str = "unknown_service";

#[derive(Debug, Default)]
pub struct MappedExport {
    pub batch: Batch,
    pub item_count: i64,
    pub rejected_count_by_reason: BTreeMap<String, i64>,
}

impl MappedExport {
    fn push_records_if_any(&mut self, records: Records) {
        if !(records.logs.is_empty() && records.spans.is_empty() && records.metrics.is_empty()) {
            self.batch.push(records);
        }
    }

    fn reject_items(&mut self, count: usize, reason: impl Into<String>) {
        *self
            .rejected_count_by_reason
            .entry(reason.into())
            .or_default() += i64::try_from(count).unwrap_or(i64::MAX);
    }

    fn reject_every_item(&mut self, count: usize, reason: impl Into<String>) {
        self.item_count += i64::try_from(count).unwrap_or(i64::MAX);
        self.reject_items(count, reason);
    }

    #[must_use]
    pub fn rejected_count(&self) -> i64 {
        self.rejected_count_by_reason.values().sum()
    }
}

#[must_use]
pub fn map_logs_request(request: ExportLogsServiceRequest) -> MappedExport {
    let mut mapped = MappedExport::default();
    for resource_logs in request.resource_logs {
        let mut records = empty_records_for_resource(resource_logs.resource);
        for scope_logs in resource_logs.scope_logs {
            let scope = scope_attributes(scope_logs.scope);
            for record in scope_logs.log_records {
                mapped.item_count += 1;
                records.logs.push(map_log_record(record, &scope));
            }
        }
        mapped.push_records_if_any(records);
    }
    mapped
}

#[must_use]
pub fn map_trace_request(request: ExportTraceServiceRequest) -> MappedExport {
    let mut mapped = MappedExport::default();
    for resource_spans in request.resource_spans {
        let mut records = empty_records_for_resource(resource_spans.resource);
        for scope_spans in resource_spans.scope_spans {
            let scope = scope_attributes(scope_spans.scope);
            for span in scope_spans.spans {
                mapped.item_count += 1;
                match map_span_with_valid_ids(span, &scope) {
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
pub fn map_metrics_request(request: ExportMetricsServiceRequest) -> MappedExport {
    let mut mapped = MappedExport::default();
    for resource_metrics in request.resource_metrics {
        let mut records = empty_records_for_resource(resource_metrics.resource);
        for scope_metrics in resource_metrics.scope_metrics {
            let scope = scope_attributes(scope_metrics.scope);
            for metric in scope_metrics.metrics {
                let descriptor = MetricDescriptor {
                    name: metric.name,
                    unit: metric.unit,
                    scope: &scope,
                };
                match metric.data {
                    Some(metric::Data::Gauge(gauge)) => {
                        add_number_points(
                            &mut mapped,
                            &mut records,
                            &descriptor,
                            NumberKind::Gauge,
                            gauge.data_points,
                        );
                    }
                    Some(metric::Data::Sum(sum)) => {
                        match number_kind_of_sum(sum.is_monotonic, sum.aggregation_temporality) {
                            Ok(kind) => {
                                add_number_points(
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
                    Some(metric::Data::Histogram(histogram)) => add_histogram_points(
                        &mut mapped,
                        &mut records,
                        &descriptor,
                        histogram.aggregation_temporality,
                        histogram
                            .data_points
                            .into_iter()
                            .map(map_explicit_histogram_point)
                            .collect(),
                    ),
                    Some(metric::Data::ExponentialHistogram(histogram)) => add_histogram_points(
                        &mut mapped,
                        &mut records,
                        &descriptor,
                        histogram.aggregation_temporality,
                        histogram
                            .data_points
                            .into_iter()
                            .map(map_exponential_histogram_point)
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

fn empty_records_for_resource(resource: Option<OtlpResource>) -> Records {
    let attributes = map_attributes(
        resource
            .map(|resource| resource.attributes)
            .unwrap_or_default(),
    );
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

fn overlay_on_scope_attributes(scope: &Attributes, record_key_values: Vec<KeyValue>) -> Attributes {
    let mut attributes = scope.clone();
    attributes.extend(map_attributes(record_key_values));
    attributes
}

fn map_log_record(record: LogRecord, scope: &Attributes) -> Log {
    let logged_at = [record.time_unix_nano, record.observed_time_unix_nano]
        .into_iter()
        .find(|&unix_nanos| unix_nanos != 0)
        .map_or_else(now_unix_nanos, clamp_to_signed_nanos);
    let body = match record.body.and_then(|body| body.value) {
        Some(any_value::Value::StringValue(body)) => body,
        None => String::new(),
        Some(other) => map_any_value(other).to_string(),
    };
    let mut attributes = overlay_on_scope_attributes(scope, record.attributes);
    if !record.event_name.is_empty() {
        attributes.insert("event.name", record.event_name);
    }
    Log {
        logged_at,
        severity: Severity::from_number(record.severity_number),
        body,
        trace_context: TraceContext::from_otlp_bytes(&record.trace_id, &record.span_id),
        attributes,
    }
}

fn map_span_with_valid_ids(span: OtlpSpan, scope: &Attributes) -> Option<Span> {
    let mut attributes = overlay_on_scope_attributes(scope, span.attributes);
    let status = span.status.unwrap_or_default();
    if !status.message.is_empty() {
        attributes.insert("otel.status_description", status.message);
    }
    let events = span
        .events
        .into_iter()
        .map(|event| SpanEvent {
            occurred_at: clamp_to_signed_nanos(event.time_unix_nano),
            name: event.name,
            attributes: map_attributes(event.attributes),
        })
        .collect();
    Some(Span {
        trace_id: TraceId::from_otlp_bytes(&span.trace_id)?,
        span_id: SpanId::from_otlp_bytes(&span.span_id)?,
        parent_span_id: SpanId::from_otlp_bytes(&span.parent_span_id),
        name: span.name,
        kind: SpanKind::from_number(span.kind),
        started_at: clamp_to_signed_nanos(span.start_time_unix_nano),
        duration_ns: clamp_to_signed_nanos(
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
    // The writer merges the metrics of equal attributes into one series.
    fn wrap_points_in_metric(&self, attributes: Vec<KeyValue>, points: Points) -> Metric {
        Metric {
            name: self.name.clone(),
            unit: self.unit.clone(),
            attributes: overlay_on_scope_attributes(self.scope, attributes),
            points,
        }
    }

    fn wrap_histogram_point_in_metric(
        &self,
        temporality: Temporality,
        point: MappedHistogramPoint,
    ) -> anyhow::Result<Metric> {
        let histogram_point = HistogramPoint {
            recorded_at: point.recorded_at,
            histogram: Histogram {
                count: point.count,
                sum: point.sum,
                min: point.min,
                max: point.max,
                buckets: point.checked_buckets?,
            },
        };
        Ok(self.wrap_points_in_metric(
            point.attributes,
            Points::Histogram(temporality, vec![histogram_point]),
        ))
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

fn map_aggregation_temporality(aggregation_temporality: i32) -> Option<Temporality> {
    match AggregationTemporality::try_from(aggregation_temporality) {
        Ok(AggregationTemporality::Cumulative) => Some(Temporality::Cumulative),
        Ok(AggregationTemporality::Delta) => Some(Temporality::Delta),
        Ok(AggregationTemporality::Unspecified) | Err(_) => None,
    }
}

fn number_kind_of_sum(
    is_monotonic: bool,
    aggregation_temporality: i32,
) -> Result<NumberKind, &'static str> {
    match (
        is_monotonic,
        map_aggregation_temporality(aggregation_temporality),
    ) {
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
    has_no_recorded_value: bool,
    attributes: Vec<KeyValue>,
    recorded_at: i64,
    count: u64,
    sum: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    checked_buckets: anyhow::Result<Buckets>,
}

fn map_explicit_histogram_point(point: HistogramDataPoint) -> MappedHistogramPoint {
    // The store needs one more count than bounds, and OTLP allows a point without buckets.
    let counts = if point.bucket_counts.is_empty() && point.explicit_bounds.is_empty() {
        vec![point.count]
    } else {
        point.bucket_counts
    };
    MappedHistogramPoint {
        has_no_recorded_value: has_no_recorded_value(point.flags),
        attributes: point.attributes,
        recorded_at: clamp_to_signed_nanos(point.time_unix_nano),
        count: point.count,
        sum: point.sum,
        min: point.min,
        max: point.max,
        checked_buckets: ExplicitBuckets::new(point.explicit_bounds, counts).map(Buckets::Explicit),
    }
}

fn map_exponential_histogram_point(point: ExponentialHistogramDataPoint) -> MappedHistogramPoint {
    MappedHistogramPoint {
        has_no_recorded_value: has_no_recorded_value(point.flags),
        attributes: point.attributes,
        recorded_at: clamp_to_signed_nanos(point.time_unix_nano),
        count: point.count,
        sum: point.sum,
        min: point.min,
        max: point.max,
        checked_buckets: ExponentialBuckets::new(
            point.scale,
            point.zero_count,
            map_indexed_counts(point.positive),
            map_indexed_counts(point.negative),
        )
        .map(Buckets::Exponential),
    }
}

fn map_indexed_counts(buckets: Option<exponential_histogram_data_point::Buckets>) -> IndexedCounts {
    buckets.map_or_else(IndexedCounts::default, |buckets| IndexedCounts {
        offset: buckets.offset,
        counts: buckets.bucket_counts,
    })
}

fn add_histogram_points(
    mapped: &mut MappedExport,
    records: &mut Records,
    descriptor: &MetricDescriptor,
    aggregation_temporality: i32,
    points: Vec<MappedHistogramPoint>,
) {
    let Some(temporality) = map_aggregation_temporality(aggregation_temporality) else {
        mapped.reject_every_item(points.len(), "a histogram needs an aggregation temporality");
        return;
    };
    for point in points {
        mapped.item_count += 1;
        if point.has_no_recorded_value {
            continue;
        }
        match descriptor.wrap_histogram_point_in_metric(temporality, point) {
            Ok(histogram_metric) => records.metrics.push(histogram_metric),
            Err(error) => mapped.reject_items(1, format!("{error:#}")),
        }
    }
}

fn add_number_points(
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
        let mapped_point = NumberPoint {
            recorded_at: clamp_to_signed_nanos(point.time_unix_nano),
            value,
        };
        records.metrics.push(
            descriptor
                .wrap_points_in_metric(point.attributes, kind.wrap_points(vec![mapped_point])),
        );
    }
}

const fn has_no_recorded_value(flags: u32) -> bool {
    flags & DataPointFlags::NoRecordedValueMask as u32 != 0
}

fn clamp_to_signed_nanos(unix_nanos: u64) -> i64 {
    i64::try_from(unix_nanos).unwrap_or(i64::MAX)
}

fn map_attributes(key_values: Vec<KeyValue>) -> Attributes {
    key_values
        .into_iter()
        .map(|key_value| {
            let value = key_value
                .value
                .and_then(|any_value| any_value.value)
                .map_or(AttributeValue::Null, map_any_value);
            (key_value.key, value)
        })
        .collect()
}

fn map_any_value(value: any_value::Value) -> AttributeValue {
    match value {
        any_value::Value::StringValue(text) => AttributeValue::String(text),
        any_value::Value::BoolValue(flag) => AttributeValue::Bool(flag),
        any_value::Value::IntValue(integer) => AttributeValue::Int(integer),
        any_value::Value::DoubleValue(double) if double.is_finite() => {
            AttributeValue::Double(double)
        }
        // JSON cannot hold NaN or an infinity.
        any_value::Value::DoubleValue(double) => AttributeValue::String(double.to_string()),
        any_value::Value::ArrayValue(array) => AttributeValue::Array(
            array
                .values
                .into_iter()
                .map(|any_value: AnyValue| {
                    any_value.value.map_or(AttributeValue::Null, map_any_value)
                })
                .collect(),
        ),
        any_value::Value::KvlistValue(list) => AttributeValue::Map(map_attributes(list.values)),
        // OTLP/JSON writes bytes as base64.
        any_value::Value::BytesValue(bytes) => AttributeValue::String(STANDARD.encode(bytes)),
        // Only profiles use the string table.
        any_value::Value::StringValueStrindex(_) => AttributeValue::Null,
    }
}
