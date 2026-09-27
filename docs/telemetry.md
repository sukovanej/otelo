---
created: 2026-09-27T18:27:04Z
parent: ./design.md
---
# Telemetry

How siner takes in logs, traces, and metrics, where it keeps them, and how a human or an agent reads them back. [[./design.md]] has the reasons for the whole product.

```mermaid
flowchart LR
  app[App with OTel SDK] -->|OTLP HTTP :4318| recv[OTLP receiver]
  app -->|OTLP gRPC :4317| recv
  journal[(journald)] --> tail[Journald tailer]
  proc[/proc, statfs, cgroup v2/] --> host[Host collector]
  recv --> writer[Writer]
  tail --> writer
  host --> writer
  writer --> day[(telemetry/YYYY-MM-DD.sqlite)]
  day --> rollup[Rollups]
  rollup --> agg[(metrics-rollup.sqlite)]
  day --> api[Query API]
  agg --> api
  api -->|HTTP| cli[siner CLI]
  api -->|HTTP| ui[UI]
```

## Rules

- One writer task owns every write. Sources send batches to it over a bounded channel. When the channel is full, the source drops the batch and counts the drop, so a burst of telemetry never takes memory from the apps.
- SQLite in WAL mode. One file per UTC day for raw data. Retention deletes whole files. The defaults are 7 days of raw data, 14 days of 1-minute rollups, and 90 days of 1-hour rollups. [[../tasks/00008-roll-up-metrics-to-1-minute-and-1.md]] explains the rollups.
- A query that spans days attaches each day file. The query API caps a range at the retention, so the attach limit is never reached.
- `service` is the OTel `service.name` resource attribute. For a journald record it is the systemd unit without `.service`.
- Journald and the host collector read Linux interfaces. On macOS they compile to nothing, and their tests read recorded fixtures.

## Schema of a day file

```mermaid
erDiagram
  resources ||--o{ logs : has
  resources ||--o{ spans : has
  resources ||--o{ series : has
  series ||--o{ points : has
  resources {
    int id PK
    text service
    text attributes "JSON, deduplicated by hash"
  }
  logs {
    int ts "unix nanos"
    int resource_id FK
    int severity "OTel severity number"
    text body "FTS5 index"
    blob trace_id
    blob span_id
    text attributes "JSON"
    text source "otlp or journald"
  }
  spans {
    blob trace_id "indexed"
    blob span_id
    blob parent_span_id
    int resource_id FK
    text name
    int kind
    int start_ts
    int duration_ns
    int status
    text attributes "JSON"
    text events "JSON"
  }
  series {
    int id PK
    int resource_id FK
    text name
    text kind "gauge, sum, histogram"
    text unit
    text labels "JSON"
  }
  points {
    int series_id FK
    int ts
    real value
    text histogram "JSON buckets, null for gauge and sum"
  }
```

## Querying

The CLI and the UI use the same HTTP query API. Every CLI command prints a table to a terminal and JSON otherwise, and `--json` and `--table` override that. Every query has a row limit. A cut result says so and names the flag that narrows it. Agents read what humans read, so the output stays small by default:

- `siner logs` groups lines by message template first, with counts, and prints samples. `--raw` prints lines.
- `siner traces` lists traces with the root span, duration, and error flag. `siner trace <id>` prints the span tree.
- `siner metrics <name>` prints a series at a step that fits the range.
- `siner sql` runs a read-only query against the day files.

Until UI auth exists, the daemon listens on `127.0.0.1` only, and a laptop reaches it through an SSH tunnel.
