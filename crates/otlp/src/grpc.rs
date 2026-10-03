use anyhow::Context;
use opentelemetry_proto::tonic::collector::logs::v1::logs_service_server::{
    LogsService, LogsServiceServer,
};
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::collector::metrics::v1::metrics_service_server::{
    MetricsService, MetricsServiceServer,
};
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tonic::codec::CompressionEncoding;
use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;
use tonic::{Request, Response, Status};

use crate::{ExportRequest, Intake, MAX_REQUEST_BYTES};

pub async fn serve_grpc(
    listener: TcpListener,
    intake: Intake,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let export_service = ExportService { intake };
    Server::builder()
        .add_service(
            LogsServiceServer::new(export_service.clone())
                .accept_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(MAX_REQUEST_BYTES),
        )
        .add_service(
            TraceServiceServer::new(export_service.clone())
                .accept_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(MAX_REQUEST_BYTES),
        )
        .add_service(
            MetricsServiceServer::new(export_service)
                .accept_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(MAX_REQUEST_BYTES),
        )
        .serve_with_incoming_shutdown(TcpIncoming::from(listener), shutdown.cancelled_owned())
        .await
        .context("serve OTLP over gRPC")
}

#[derive(Clone)]
struct ExportService {
    intake: Intake,
}

impl ExportService {
    async fn accept_export<R: ExportRequest>(
        &self,
        request: Request<R>,
    ) -> Result<Response<R::Response>, Status> {
        self.intake
            .accept_export(request.into_inner())
            .await
            .map(Response::new)
            .map_err(|error| Status::unavailable(format!("{error:#}")))
    }
}

#[tonic::async_trait]
impl LogsService for ExportService {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        self.accept_export(request).await
    }
}

#[tonic::async_trait]
impl TraceService for ExportService {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        self.accept_export(request).await
    }
}

#[tonic::async_trait]
impl MetricsService for ExportService {
    async fn export(
        &self,
        request: Request<ExportMetricsServiceRequest>,
    ) -> Result<Response<ExportMetricsServiceResponse>, Status> {
        self.accept_export(request).await
    }
}
