---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00004-receive-otlp-over-http.md
- ./00009-serve-the-query-api.md
tags:
- feature
---
# Send mudro's traces to a local siner

The first real client. Run `siner serve` on the laptop and `mise dev` in conquer with `OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318` and no Better Stack headers. Play a match. Then `siner traces --service mudro` lists the requests and the match events, and `siner trace <id>` shows the SQLite spans under one of them. Write down each thing in mudro's spans that siner shows badly.
