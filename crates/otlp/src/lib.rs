//! The OTLP receiver: logs, traces, and metrics over HTTP and gRPC.
//!
//! Both transports decode an export request, map it to the rows of the store
//! with [`map`], and hand the rows to the telemetry writer in one batch. When
//! the writer's channel is full, the whole batch is dropped, and the response
//! says so in `partial_success`, as the OTLP spec asks.

mod grpc;
mod http;
pub mod map;

use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsPartialSuccess, ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsPartialSuccess, ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTracePartialSuccess, ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use siner_telemetry::Sender;

pub use grpc::serve as serve_grpc;
pub use http::serve as serve_http;

use crate::map::Mapped;

/// The largest export request either transport takes, after gzip. It is the
/// gRPC default, and an SDK batch is a few hundred kilobytes.
const MAX_REQUEST: usize = 4 * 1024 * 1024;

/// An export request of one signal.
trait Export: prost::Message + DeserializeOwned + Default + 'static {
    type Response: prost::Message + Serialize;

    /// Maps the request, sends the rows to the writer, and answers.
    fn receive(self, sender: &Sender) -> Self::Response;
}

impl Export for ExportLogsServiceRequest {
    type Response = ExportLogsServiceResponse;

    fn receive(self, sender: &Sender) -> Self::Response {
        ExportLogsServiceResponse {
            partial_success: send(sender, map::logs(self)).map(|(rejected, message)| {
                ExportLogsPartialSuccess {
                    rejected_log_records: rejected,
                    error_message: message,
                }
            }),
        }
    }
}

impl Export for ExportTraceServiceRequest {
    type Response = ExportTraceServiceResponse;

    fn receive(self, sender: &Sender) -> Self::Response {
        ExportTraceServiceResponse {
            partial_success: send(sender, map::spans(self)).map(|(rejected, message)| {
                ExportTracePartialSuccess {
                    rejected_spans: rejected,
                    error_message: message,
                }
            }),
        }
    }
}

impl Export for ExportMetricsServiceRequest {
    type Response = ExportMetricsServiceResponse;

    fn receive(self, sender: &Sender) -> Self::Response {
        ExportMetricsServiceResponse {
            partial_success: send(sender, map::metrics(self)).map(|(rejected, message)| {
                ExportMetricsPartialSuccess {
                    rejected_data_points: rejected,
                    error_message: message,
                }
            }),
        }
    }
}

/// Sends the rows to the writer. Returns how many items were rejected and
/// why, or `None` when every item was taken.
fn send(sender: &Sender, mapped: Mapped) -> Option<(i64, String)> {
    let Mapped {
        batch,
        items,
        rejected,
        reasons,
    } = mapped;
    if !batch.is_empty() && !sender.send(batch) {
        let message = "the telemetry queue of siner is full, so it dropped the whole request";
        return Some((items, message.into()));
    }
    (rejected > 0).then(|| (rejected, reasons.into_iter().collect::<Vec<_>>().join("; ")))
}
