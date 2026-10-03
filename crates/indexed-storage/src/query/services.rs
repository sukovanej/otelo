use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{Attributes, SpanKind};

/// The services that sent spans or logs in a range, the busiest first.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Services {
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub end_at: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    pub services: Vec<ServiceSummary>,
    /// More services sent telemetry than the limit let through.
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ServiceSummary {
    pub service: String,
    /// The attributes of the newest resource of the service that has any,
    /// such as `telemetry.sdk.language`.
    pub resource: Attributes,
    pub stats: ServiceStats,
    /// Every step of the range, oldest first.
    pub buckets: Vec<ServiceBucket>,
}

/// One service in a range: its stats over time and by operation.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Service {
    pub service: String,
    /// The attributes of the newest resource of the service. Empty when the
    /// range has none of its telemetry.
    pub resource: Attributes,
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub end_at: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    pub stats: ServiceStats,
    /// Every step of the range, oldest first.
    pub buckets: Vec<ServiceBucket>,
    /// The requests by span name, the most first.
    pub operations: Vec<Operation>,
    /// The service has more operations than the limit let through.
    pub truncated: bool,
}

/// The requests and the logs of a service in a range.
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct ServiceStats {
    pub requests: Requests,
    pub logs: u64,
    /// The logs of the error level and above.
    pub error_logs: u64,
}

/// The requests and the logs of a service in one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ServiceBucket {
    /// The start of the step.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    pub requests: Requests,
    pub logs: u64,
    pub error_logs: u64,
}

/// The requests of a service by the name of their span.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Operation {
    /// The span name, such as `GET /users/{id}`.
    pub name: String,
    /// The OpenTelemetry span kind.
    #[schema(value_type = i32)]
    pub kind: SpanKind,
    /// The attributes of its newest request, which tell what the operation
    /// is, such as `http.request.method` and `http.route`.
    pub attributes: Attributes,
    pub requests: Requests,
}

/// One operation of a service in a range: its requests over the range and
/// in buckets of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct OperationDetail {
    pub service: String,
    /// The span name.
    pub name: String,
    /// The OpenTelemetry span kind.
    #[schema(value_type = i32)]
    pub kind: SpanKind,
    /// The attributes of its newest request. Empty when the range has none.
    pub attributes: Attributes,
    /// The range, after the retention capped it.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub end_at: Timestamp,
    /// The length of a bucket in nanoseconds.
    pub step_ns: i64,
    pub requests: Requests,
    /// Every step of the range, oldest first.
    pub buckets: Vec<RequestBucket>,
}

/// The requests of one step.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RequestBucket {
    /// The start of the step.
    #[schema(value_type = String, format = DateTime)]
    pub start_at: Timestamp,
    pub requests: Requests,
}

/// Spans that enter a service: roots, and spans of the server or the
/// consumer kind.
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct Requests {
    pub count: u64,
    /// The failed ones.
    pub errors: u64,
    /// Their durations added up.
    pub total_ns: i64,
    /// Estimates of the percentiles of their durations, off by 1% at most.
    /// `None` without requests.
    #[schema(required = true)]
    pub latency: Option<Latency>,
}

/// Percentiles of durations, in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Latency {
    pub p50: i64,
    pub p95: i64,
    pub p99: i64,
}
