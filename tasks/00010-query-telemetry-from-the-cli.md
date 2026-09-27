---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00009-serve-the-query-api.md
tags:
- feature
---
# Query telemetry from the CLI

`siner logs`, `siner traces`, `siner trace <id>`, `siner metrics [name]`, and `siner sql`, as [[../docs/telemetry.md]] describes them.

- A table when stdout is a terminal, JSON otherwise. `--json` and `--table` override.
- `--since 1h` style ranges, default 1 hour.
- A cut result prints one line that names the flag that narrows it.
- The daemon address comes from `--daemon` or `SINER_URL`, default `http://127.0.0.1:7070`.
- `siner trace` prints the span tree with durations, and the error spans marked.
