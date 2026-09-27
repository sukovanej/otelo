---
status: backlog
created: 2026-09-27T18:27:45Z
parent: '1'
dependencies:
- '2'
- '3'
tags:
- feature
---
# Query telemetry through the API and the CLI

HTTP endpoints on the daemon, and the CLI commands on top of them. The UI uses the same endpoints later. [[../docs/telemetry.md]] describes the commands.

## API

Every endpoint takes a time range (`since`, `until`) and a `limit`, and says when it cut the result.

- Logs: filter by service, severity, text (FTS5), and trace ID. Two forms: raw lines, and groups by message template with a count and samples. The template replaces numbers, UUIDs, hex IDs, and quoted strings with placeholders.
- Traces: list roots with service, name, duration, span count, and an error flag. Filter by service, name, minimum duration, and errors only.
- One trace: every span with its parent, and the logs that carry its trace ID.
- Metrics: the names with their labels, and one series at a step.
- SQL: a read-only query over the attached day files, with a row limit and a time limit.

An OpenAPI spec comes out of the code, so the UI can generate its client later.

## CLI

`siner logs`, `siner traces`, `siner trace <id>`, `siner metrics [name]`, and `siner sql`.

- A table when stdout is a terminal, JSON otherwise. `--json` and `--table` override.
- `--since 1h` style ranges, default 1 hour.
- A cut result prints one line that names the flag that narrows it.
- The daemon address comes from `--daemon` or `SINER_URL`, default `http://127.0.0.1:7070`.
- `siner trace` prints the span tree with durations, and marks the error spans.

## Comments

### 2026-09-27T20:22:56Z by Milan Suk via claude-code

> The reader's views (logs, spans, …) union the day files, but FTS5 does not: search each "<day>".logs_fts and join on that file's rowid. Ids count per file, so every join on resource_id or series_id also matches day.
