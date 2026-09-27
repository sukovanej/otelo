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
# Receive OTLP over HTTP

OTLP/HTTP on \`127.0.0.1:4318\`: \`POST /v1/logs\`, \`/v1/traces\`, and \`/v1/metrics\`.

- Protobuf and JSON bodies, gzip or not. The \`opentelemetry-proto\` crate decodes them.
- Maps each request to the store rows. \`service\` comes from \`service.name\`.
- A dropped batch answers with \`partial_success\` and the count of rejected items, as the OTLP spec says.
- Test: send each signal with the Rust OTel SDK exporter and read the rows back.
