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
use otelo_storage::BatchSender;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use grpc::serve_grpc;
pub use http::serve_http;

use crate::map::{MappedExport, map_logs_request, map_metrics_request, map_trace_request};

// The gRPC default, after gzip. An SDK batch is a few hundred kilobytes.
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

trait ExportRequest: prost::Message + DeserializeOwned + Default + 'static {
    type Response: prost::Message + Serialize;

    fn store_and_respond(self, sender: &BatchSender) -> Self::Response;
}

impl ExportRequest for ExportLogsServiceRequest {
    type Response = ExportLogsServiceResponse;

    fn store_and_respond(self, sender: &BatchSender) -> Self::Response {
        ExportLogsServiceResponse {
            partial_success: send_batch_to_writer(sender, map_logs_request(self)).map(
                |rejection| ExportLogsPartialSuccess {
                    rejected_log_records: rejection.rejected_count,
                    error_message: rejection.error_message,
                },
            ),
        }
    }
}

impl ExportRequest for ExportTraceServiceRequest {
    type Response = ExportTraceServiceResponse;

    fn store_and_respond(self, sender: &BatchSender) -> Self::Response {
        ExportTraceServiceResponse {
            partial_success: send_batch_to_writer(sender, map_trace_request(self)).map(
                |rejection| ExportTracePartialSuccess {
                    rejected_spans: rejection.rejected_count,
                    error_message: rejection.error_message,
                },
            ),
        }
    }
}

impl ExportRequest for ExportMetricsServiceRequest {
    type Response = ExportMetricsServiceResponse;

    fn store_and_respond(self, sender: &BatchSender) -> Self::Response {
        ExportMetricsServiceResponse {
            partial_success: send_batch_to_writer(sender, map_metrics_request(self)).map(
                |rejection| ExportMetricsPartialSuccess {
                    rejected_data_points: rejection.rejected_count,
                    error_message: rejection.error_message,
                },
            ),
        }
    }
}

struct Rejection {
    rejected_count: i64,
    error_message: String,
}

fn send_batch_to_writer(sender: &BatchSender, mapped: MappedExport) -> Option<Rejection> {
    let rejected_count = mapped.rejected_count();
    let MappedExport {
        batch,
        item_count,
        rejected_count_by_reason,
    } = mapped;
    if !batch.is_empty() && !sender.send_batch(batch) {
        return Some(Rejection {
            rejected_count: item_count,
            error_message: "the telemetry queue of otelo is full, so it dropped the whole request"
                .into(),
        });
    }
    (rejected_count > 0).then(|| Rejection {
        rejected_count,
        error_message: rejected_count_by_reason
            .into_keys()
            .collect::<Vec<_>>()
            .join("; "),
    })
}
