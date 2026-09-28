//! OTLP over gRPC: the logs, trace, and metrics collector services.

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
use siner_telemetry::Sender;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tonic::codec::CompressionEncoding;
use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;
use tonic::{Request, Response, Status};

use crate::{Export, MAX_REQUEST};

/// Serves the OTLP collector services on `listener` until `shutdown` is
/// cancelled.
///
/// # Errors
///
/// When the server fails.
pub async fn serve(
    listener: TcpListener,
    sender: Sender,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let receiver = Receiver(sender);
    Server::builder()
        .add_service(
            LogsServiceServer::new(receiver.clone())
                .accept_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(MAX_REQUEST),
        )
        .add_service(
            TraceServiceServer::new(receiver.clone())
                .accept_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(MAX_REQUEST),
        )
        .add_service(
            MetricsServiceServer::new(receiver)
                .accept_compressed(CompressionEncoding::Gzip)
                .max_decoding_message_size(MAX_REQUEST),
        )
        .serve_with_incoming_shutdown(TcpIncoming::from(listener), shutdown.cancelled_owned())
        .await
        .context("serve OTLP over gRPC")
}

#[derive(Clone)]
struct Receiver(Sender);

#[tonic::async_trait]
impl LogsService for Receiver {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        Ok(Response::new(request.into_inner().receive(&self.0)))
    }
}

#[tonic::async_trait]
impl TraceService for Receiver {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        Ok(Response::new(request.into_inner().receive(&self.0)))
    }
}

#[tonic::async_trait]
impl MetricsService for Receiver {
    async fn export(
        &self,
        request: Request<ExportMetricsServiceRequest>,
    ) -> Result<Response<ExportMetricsServiceResponse>, Status> {
        Ok(Response::new(request.into_inner().receive(&self.0)))
    }
}
