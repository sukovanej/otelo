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
use otelo_storage::Sender;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use grpc::serve_grpc;
pub use http::serve_http;

use crate::map::MappedExport;

// The gRPC default, after gzip. An SDK batch is a few hundred kilobytes.
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

trait ExportRequest: prost::Message + DeserializeOwned + Default + 'static {
    type Response: prost::Message + Serialize;

    fn store_and_respond(self, sender: &Sender) -> Self::Response;
}

impl ExportRequest for ExportLogsServiceRequest {
    type Response = ExportLogsServiceResponse;

    fn store_and_respond(self, sender: &Sender) -> Self::Response {
        ExportLogsServiceResponse {
            partial_success: send_rows_to_writer(sender, map::logs(self)).map(
                |(rejected, message)| ExportLogsPartialSuccess {
                    rejected_log_records: rejected,
                    error_message: message,
                },
            ),
        }
    }
}

impl ExportRequest for ExportTraceServiceRequest {
    type Response = ExportTraceServiceResponse;

    fn store_and_respond(self, sender: &Sender) -> Self::Response {
        ExportTraceServiceResponse {
            partial_success: send_rows_to_writer(sender, map::spans(self)).map(
                |(rejected, message)| ExportTracePartialSuccess {
                    rejected_spans: rejected,
                    error_message: message,
                },
            ),
        }
    }
}

impl ExportRequest for ExportMetricsServiceRequest {
    type Response = ExportMetricsServiceResponse;

    fn store_and_respond(self, sender: &Sender) -> Self::Response {
        ExportMetricsServiceResponse {
            partial_success: send_rows_to_writer(sender, map::metrics(self)).map(
                |(rejected, message)| ExportMetricsPartialSuccess {
                    rejected_data_points: rejected,
                    error_message: message,
                },
            ),
        }
    }
}

fn send_rows_to_writer(sender: &Sender, mapped: MappedExport) -> Option<(i64, String)> {
    let MappedExport {
        batch,
        item_count,
        rejected_count,
        rejection_reasons,
    } = mapped;
    if !batch.is_empty() && !sender.send(batch) {
        let message = "the telemetry queue of otelo is full, so it dropped the whole request";
        return Some((item_count, message.into()));
    }
    (rejected_count > 0).then(|| {
        (
            rejected_count,
            rejection_reasons.into_iter().collect::<Vec<_>>().join("; "),
        )
    })
}
