---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00002-run-the-daemon-with-siner-serve.md
- ./00003-store-telemetry-in-daily-sqlite-f.md
tags:
- feature
---
# Receive OTLP over HTTP and gRPC

The OTLP receiver takes logs, traces, and metrics over both transports the OTel SDKs use.

- HTTP on `127.0.0.1:4318`: `POST /v1/logs`, `/v1/traces`, and `/v1/metrics`. Protobuf and JSON bodies, gzip or not.
- gRPC on `127.0.0.1:4317` with `tonic`: the three OTLP collector services.
- The `opentelemetry-proto` crate decodes both. Both transports call one mapping from OTLP to the store rows, so a signal lands the same way whichever transport sent it.
- `service` comes from the `service.name` resource attribute.
- A dropped batch answers with `partial_success` and the count of rejected items, as the OTLP spec says.
- Test: send each signal with the Rust OTel SDK exporter over each transport and read the rows back.
