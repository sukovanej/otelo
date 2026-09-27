---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00004-receive-otlp-over-http.md
tags:
- feature
---
# Receive OTLP over gRPC

OTLP/gRPC on `127.0.0.1:4317` for the same three signals, with `tonic`. It uses the mapping of [[./00004-receive-otlp-over-http.md]]. Test: the same SDK test as over HTTP, with the gRPC exporter.
