use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::SpanStats;
use crate::Attributes;

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
    /// The attributes of the resource of the newest log or span of the
    /// service, preferring a resource that has any, such as
    /// `telemetry.sdk.language`.
    pub resource: Attributes,
    pub stats: ServiceStats,
    /// Every step of the range, oldest first.
    pub buckets: Vec<ServiceBucket>,
}

/// One service in a range: its stats over time.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Service {
    pub service: String,
    /// The attributes of the resource of the newest log or span of the
    /// service, preferring a resource that has any. Empty when the service
    /// sent no log or span.
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
}

/// The requests and the logs of a service in a range. A request is a span of
/// the server kind with `http.request.method`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct ServiceStats {
    pub requests: SpanStats,
    /// Every span of the service, of any kind.
    pub spans: u64,
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
    pub requests: SpanStats,
    pub logs: u64,
    pub error_logs: u64,
}
