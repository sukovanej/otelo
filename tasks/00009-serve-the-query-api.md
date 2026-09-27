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
# Serve the query API

HTTP endpoints on the daemon that the CLI and the UI share. Every endpoint takes a time range (`since`, `until`) and a `limit`, and says when it cut the result.

- Logs: filter by service, severity, text (FTS5), and trace ID. Two forms: raw lines, and groups by message template with a count and samples. The template replaces numbers, UUIDs, hex IDs, and quoted strings with placeholders.
- Traces: list roots with service, name, duration, span count, and an error flag. Filter by service, name, minimum duration, and errors only.
- One trace: every span with its parent, and the logs that carry its trace ID.
- Metrics: the names with their labels, and one series at a step.
- SQL: a read-only query over the attached day files, with a row limit and a time limit.

An OpenAPI spec comes out of the code, so the UI can generate its client later.
