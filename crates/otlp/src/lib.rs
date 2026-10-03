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
use std::time::Instant;

use otelo_indexed_storage::{PipelineMeters, now_unix_nanos};
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

    fn respond_with_rejections(self) -> Self::Response;
}

#[derive(Clone)]
pub struct Intake {
    journal: Arc<dyn Journal>,
    meters: Arc<PipelineMeters>,
}

impl Intake {
    #[must_use]
    pub fn new(journal: Arc<dyn Journal>, meters: Arc<PipelineMeters>) -> Self {
        Self { journal, meters }
    }

    pub async fn accept_export<R: ExportRequest>(&self, request: R) -> anyhow::Result<R::Response> {
        let encoded_request = request.encode_to_vec();
        let appended_at = Instant::now();
        let journaled =
            match self
                .journal
                .append_frame(R::SIGNAL, now_unix_nanos(), &encoded_request)
            {
                Ok(ticket) => ticket.wait_until_synced().await,
                Err(error) => Err(error),
            };
        if let Err(error) = journaled {
            self.meters.count_refused_request(R::SIGNAL);
            tracing::warn!("journal an export request: {error:#}");
            return Err(error);
        }
        self.meters
            .record_journal_sync_wait(R::SIGNAL, appended_at.elapsed());
        self.meters
            .count_journaled_request(R::SIGNAL, encoded_request.len() as u64);
        Ok(request.respond_with_rejections())
    }
}

impl ExportRequest for ExportLogsServiceRequest {
    const SIGNAL: Signal = Signal::Logs;

    type Response = ExportLogsServiceResponse;

    fn respond_with_rejections(self) -> Self::Response {
        ExportLogsServiceResponse {
            partial_success: find_rejection(&map_logs_request(self)).map(|rejection| {
                ExportLogsPartialSuccess {
                    rejected_log_records: rejection.rejected_count,
                    error_message: rejection.error_message,
                }
            }),
        }
    }
}

impl ExportRequest for ExportTraceServiceRequest {
    const SIGNAL: Signal = Signal::Spans;

    type Response = ExportTraceServiceResponse;

    fn respond_with_rejections(self) -> Self::Response {
        ExportTraceServiceResponse {
            partial_success: find_rejection(&map_trace_request(self)).map(|rejection| {
                ExportTracePartialSuccess {
                    rejected_spans: rejection.rejected_count,
                    error_message: rejection.error_message,
                }
            }),
        }
    }
}

impl ExportRequest for ExportMetricsServiceRequest {
    const SIGNAL: Signal = Signal::Metrics;

    type Response = ExportMetricsServiceResponse;

    fn respond_with_rejections(self) -> Self::Response {
        ExportMetricsServiceResponse {
            partial_success: find_rejection(&map_metrics_request(self)).map(|rejection| {
                ExportMetricsPartialSuccess {
                    rejected_data_points: rejection.rejected_count,
                    error_message: rejection.error_message,
                }
            }),
        }
    }
}

struct Rejection {
    rejected_count: i64,
    error_message: String,
}

fn find_rejection(mapped: &MappedExport) -> Option<Rejection> {
    let rejected_count = mapped.rejected_count();
    (rejected_count > 0).then(|| Rejection {
        rejected_count,
        error_message: mapped
            .rejected_count_by_reason
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join("; "),
    })
}
