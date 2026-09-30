use std::io::Read;

use anyhow::Context;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::header::{CONTENT_ENCODING, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use flate2::read::GzDecoder;
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use otelo_storage::Sender;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::{ExportRequest, MAX_REQUEST_BYTES};

pub async fn serve_http(
    listener: TcpListener,
    sender: Sender,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    axum::serve(listener, router(sender))
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await
        .context("serve OTLP over HTTP")
}

fn router(sender: Sender) -> Router {
    Router::new()
        .route("/v1/logs", post(export::<ExportLogsServiceRequest>))
        .route("/v1/traces", post(export::<ExportTraceServiceRequest>))
        .route("/v1/metrics", post(export::<ExportMetricsServiceRequest>))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .with_state(sender)
}

async fn export<R: ExportRequest>(
    State(sender): State<Sender>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let format = match Format::of(&headers) {
        Ok(format) => format,
        Err(refusal) => return refusal.respond(Format::Protobuf),
    };
    match decompress_body(&headers, body).and_then(|body| format.decode::<R>(&body)) {
        Ok(request) => format.encode(StatusCode::OK, &request.store_and_respond(&sender)),
        Err(refusal) => refusal.respond(format),
    }
}

#[derive(Clone, Copy)]
enum Format {
    Protobuf,
    Json,
}

impl Format {
    const fn content_type(self) -> &'static str {
        match self {
            Self::Protobuf => "application/x-protobuf",
            Self::Json => "application/json",
        }
    }

    fn of(headers: &HeaderMap) -> Result<Self, Refusal> {
        let media_type = headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .unwrap_or_default();
        [Self::Protobuf, Self::Json]
            .into_iter()
            .find(|format| media_type.eq_ignore_ascii_case(format.content_type()))
            .ok_or_else(|| {
                Refusal::unsupported(format!(
                    "the content type has to be application/x-protobuf or application/json, not {media_type:?}"
                ))
            })
    }

    fn decode<R: ExportRequest>(self, body: &[u8]) -> Result<R, Refusal> {
        match self {
            Self::Protobuf => R::decode(body).map_err(|error| Refusal::bad(error.to_string())),
            Self::Json => {
                let mut value = serde_json::from_slice(body)
                    .map_err(|error| Refusal::bad(error.to_string()))?;
                fit_json_to_decoder(&mut value);
                R::deserialize(value).map_err(|error| Refusal::bad(error.to_string()))
            }
        }
    }

    fn encode<M: prost::Message + Serialize>(self, status: StatusCode, message: &M) -> Response {
        let body = match self {
            Self::Protobuf => message.encode_to_vec(),
            Self::Json => serde_json::to_vec(message).expect("an OTLP message serializes"),
        };
        (
            status,
            [(CONTENT_TYPE, HeaderValue::from_static(self.content_type()))],
            body,
        )
            .into_response()
    }
}

// Proto3 JSON writes an int64 as a string, but the decoder of opentelemetry-proto
// takes only a number for `asInt`, and would lose the value.
fn fit_json_to_decoder(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if key == "exponentialHistogram" {
                    insert_missing_fields_of_exponential_points(value);
                }
                match value {
                    Value::String(text) if key == "asInt" => {
                        if let Ok(int) = text.parse::<i64>() {
                            *value = int.into();
                        }
                    }
                    _ => fit_json_to_decoder(value),
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(fit_json_to_decoder),
        _ => {}
    }
}

// Proto3 JSON leaves out a field at its default, but the decoder of opentelemetry-proto takes
// a point of an exponential histogram only with every field, and would lose the metric.
fn insert_missing_fields_of_exponential_points(histogram: &mut Value) {
    let points = histogram
        .get_mut("dataPoints")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object_mut);
    for point in points {
        let defaults = [
            ("attributes", json!([])),
            ("startTimeUnixNano", json!("0")),
            ("timeUnixNano", json!("0")),
            ("count", json!("0")),
            ("scale", json!(0)),
            ("zeroCount", json!("0")),
            ("flags", json!(0)),
            ("exemplars", json!([])),
            ("zeroThreshold", json!(0.0)),
        ];
        for (field, default) in defaults {
            point.entry(field).or_insert(default);
        }
        for side in ["positive", "negative"] {
            if let Some(buckets) = point.get_mut(side).and_then(Value::as_object_mut) {
                buckets.entry("offset").or_insert(json!(0));
                buckets.entry("bucketCounts").or_insert(json!([]));
            }
        }
    }
}

fn decompress_body(headers: &HeaderMap, body: Bytes) -> Result<Bytes, Refusal> {
    let encoding = headers
        .get(CONTENT_ENCODING)
        .map(|value| value.to_str().unwrap_or_default().trim());
    match encoding {
        None => Ok(body),
        Some(encoding) if encoding.eq_ignore_ascii_case("identity") => Ok(body),
        Some(encoding) if encoding.eq_ignore_ascii_case("gzip") => {
            let mut inflated = Vec::new();
            GzDecoder::new(&body[..])
                .take(MAX_REQUEST_BYTES as u64 + 1)
                .read_to_end(&mut inflated)
                .map_err(|error| Refusal::bad(format!("inflate the gzip body: {error}")))?;
            if inflated.len() > MAX_REQUEST_BYTES {
                return Err(Refusal {
                    status: StatusCode::PAYLOAD_TOO_LARGE,
                    code: tonic::Code::ResourceExhausted,
                    message: format!("the request is over {MAX_REQUEST_BYTES} bytes after gzip"),
                });
            }
            Ok(inflated.into())
        }
        Some(encoding) => Err(Refusal::unsupported(format!(
            "the content encoding has to be gzip or none, not {encoding:?}"
        ))),
    }
}

struct Refusal {
    status: StatusCode,
    code: tonic::Code,
    message: String,
}

impl Refusal {
    const fn bad(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: tonic::Code::InvalidArgument,
            message,
        }
    }

    const fn unsupported(message: String) -> Self {
        Self {
            status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            code: tonic::Code::InvalidArgument,
            message,
        }
    }

    // OTLP/HTTP answers a refused request with a google.rpc.Status.
    fn respond(self, format: Format) -> Response {
        let status = RpcStatus {
            code: self.code as i32,
            message: self.message,
        };
        format.encode(self.status, &status)
    }
}

#[derive(Clone, PartialEq, Eq, prost::Message, Serialize)]
struct RpcStatus {
    #[prost(int32, tag = "1")]
    code: i32,
    #[prost(string, tag = "2")]
    message: String,
}
