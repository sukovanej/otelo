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
use std::sync::Arc;

use otelo_indexed_storage::{BatchSender, now_unix_nanos};
use otelo_journal::Journal;
use otelo_query::Signal;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use grpc::serve_grpc;
pub use http::serve_http;

use crate::map::{MappedExport, map_logs_request, map_metrics_request, map_trace_request};

// The gRPC default, after gzip. An SDK batch is a few hundred kilobytes.
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

pub trait ExportRequest: prost::Message + DeserializeOwned + Default + 'static {
    const SIGNAL: Signal;

    type Response: prost::Message + Serialize;

    fn send_to_writer_and_respond(self, sender: &BatchSender) -> Self::Response;
}

#[derive(Clone)]
pub struct Intake {
    journal: Arc<dyn Journal>,
    sender: BatchSender,
}

impl Intake {
    #[must_use]
    pub fn new(journal: Arc<dyn Journal>, sender: BatchSender) -> Self {
        Self { journal, sender }
    }

    pub async fn accept_export<R: ExportRequest>(&self, request: R) -> anyhow::Result<R::Response> {
        self.journal
            .append_frame(R::SIGNAL, now_unix_nanos(), &request.encode_to_vec())?
            .wait_until_synced()
            .await
            .inspect_err(|error| tracing::warn!("journal an export request: {error:#}"))?;
        Ok(request.send_to_writer_and_respond(&self.sender))
    }
}

impl ExportRequest for ExportLogsServiceRequest {
    const SIGNAL: Signal = Signal::Logs;

    type Response = ExportLogsServiceResponse;

    fn send_to_writer_and_respond(self, sender: &BatchSender) -> Self::Response {
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
    const SIGNAL: Signal = Signal::Spans;

    type Response = ExportTraceServiceResponse;

    fn send_to_writer_and_respond(self, sender: &BatchSender) -> Self::Response {
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
    const SIGNAL: Signal = Signal::Metrics;

    type Response = ExportMetricsServiceResponse;

    fn send_to_writer_and_respond(self, sender: &BatchSender) -> Self::Response {
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
