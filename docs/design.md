---
created: 2026-09-27T18:10:53Z
---
# Design

Otelo receives, keeps, and shows the OpenTelemetry logs, traces, and metrics of the apps on one Linux or macOS server. It is one binary. The same binary is the daemon on the server and the CLI on a laptop or in CI.

The first user is mudro (conquer) on a DigitalOcean droplet: Ubuntu 24.04, 1 vCPU, 1 GB of memory. mudro, Caddy, and otelo share that memory, so otelo targets about 50 MB RSS.

## Decisions

- OpenTelemetry only. Apps send OTLP, and otelo stores it and answers queries about it. Otelo does not deploy, start, or supervise apps, and it keeps no secrets. The app's deploy script and the service manager of the OS keep doing that.
- Logs of apps without OTel, such as Caddy, reach otelo through an OTel collector that sends OTLP. Otelo reads no journald and no log files.
- One server only. Multi-server is out of scope, and the README says so. The code can assume one SQLite and one clock.
- Otelo does not own Caddy. Caddy puts the UI on a hostname with HTTPS and proxies it to the daemon.
- A CLI, not MCP. Humans and agents use the same commands. The CLI prints a table to a terminal and JSON otherwise; `--json` and `--table` override. Every query has a row limit and says when it truncated. `otelo guide` prints how to debug with otelo ([[../tasks/00011-print-a-debugging-guide-with-sin.md]]).
- No alerts for now.

## Parts of the daemon

- OTLP receiver on 4317 (gRPC) and 4318 (HTTP) for logs, traces, and metrics.
- Host collector: every 15 seconds, the CPU, load, memory, swap, disks, and network of the machine, the CPU and memory of every service the service manager runs, and the CPU, memory, and data size of otelo itself ([[../tasks/00007-collect-host-and-service-metrics.md]]). It needs no configuration. [[./telemetry.md]] lists the metrics, and [[./platforms.md]] says where each number comes from.
- The daemon's own telemetry, which it sends to its own receiver under the service `otelo`.
- Query API over HTTP, which the CLI and the UI both read. [[./telemetry.md]] has the queries.
- UI: a SolidJS SPA, built with Vite, in a pnpm workspace under `packages/`: `app` is the SPA, `ui` its components, its Tailwind v4 theme of colors and type, and its fonts, and `viz` the parts that show data: a chart of values over time, a table, a stat, a sparkline, and the panel that frames them. The pages build on `viz`, and so will custom dashboards, so its props are plain data and name units (`count`, `duration`, `ratio`, `rate`, `bytes`) and series colors (`series-1` to `series-8`, `error`, `p95`) by string, which a dashboard can keep as JSON. The UI ships IBM Plex Sans and IBM Plex Mono, so it looks the same on macOS, Windows, and Linux. A release binary embeds `packages/app/dist`, and the daemon serves it on the address of the API. Any path outside `/api` gets `index.html`.

## Code

The Rust workspace in `crates/` has one crate per part, and the `otelo` binary puts them together:

| Crate | What it holds |
|---|---|
| `otelo-query` | the query language: its parser and its completion |
| `otelo-storage` | the storage interface: the model of the records, what the queries return, the channel to the writer, and the `Storage` and `RangeQueries` traits |
| `otelo-storage-sqlite` | the SQLite backend: the day files, the writer, the queries, and the indexed attributes in the state file, `state.sqlite` |
| `otelo-journal` | the journal of OTLP requests: its frames, its segments, their compression, and the retention ([[../tasks/00023-journal-the-otlp-that-otelo-rece.md]]) |
| `otelo-otlp` | the OTLP receiver over HTTP and gRPC |
| `otelo-host` | the host collector: the readers of the machine, of its services, and of otelo itself, and the mapping from what they read to metric points |
| `otelo-api` | the HTTP API, its errors, and its OpenAPI spec |
| `otelo` | the binary: `otelo serve`, the daemon's own telemetry, the web UI, and the CLI commands in `cli/` |

## Storage

A journal and an index ([[../tasks/00021-rebuild-the-index-from-a-journal.md]]).

- The journal keeps the OTLP export requests as protobuf, in hourly segments per signal, compressed with zstd once the hour ends. It is the only telemetry that has to outlive a change of the storage, and it keeps 30 days by default.
- The index is one SQLite file, `telemetry.sqlite`, that an indexer builds from the journal: FTS5 for log search, spans indexed by `trace_id`, metrics in a `series` table and a narrow `points` table, and their 1-minute and 1-hour rollups ([[../tasks/00008-roll-up-metrics-to-1-minute-and-1.md]]). Each signal has its own retention, 7 days by default, and retention deletes rows.
- The index carries a storage version. After a change of the storage, the daemon does not start on an index of another version, and `otelo reindex` rebuilds it from the journal.
- One state file for the indexed attributes, and later the users and tokens. A journal cannot rebuild it, so it has migrations ([[../tasks/00013-authenticate-with-passkeys-and-s.md]]).

## UI and CLI auth

Passkeys for the UI, and scoped tokens for the CLI and agents. [[../tasks/00013-authenticate-with-passkeys-and-s.md]] has the plan. Until it lands, the daemon listens on `127.0.0.1` only, and a laptop reaches it through an SSH tunnel.

## What otelo replaces in conquer

| Today | With otelo |
|---|---|
| OTLP traces to Better Stack | OTLP to `127.0.0.1:4318` |
| Vector to Better Stack | OTel logs from mudro, and the host collector |

mudro changes: point `OTEL_EXPORTER_OTLP_ENDPOINT` at `http://127.0.0.1:4318` and turn on OTel logs, so log lines reach otelo and carry trace IDs. The deploy with `release.sh`, the units from `provision.sh`, the secrets in `/etc/mudro/env`, and the Better Stack heartbeat of the backup stay as they are.

## Out of scope

The first design of this project, under the name siner, also deployed and ran the apps. These parts are out now:

- Deploys from a tarball, with a drain hook and a rollback when the health check fails.
- Secrets encrypted with age, and `exec`, which started an app with them.
- Writing a systemd unit or a launchd plist per app.
- Health checks and heartbeats.
- The timeline of deploys, restarts, and OOM kills.
- `siner.json` in the app repository.

A tool that does these could send its events to otelo over OTLP, so otelo shows them next to the telemetry.

## Order

1. OTLP receiver, day files, the query API and CLI, and the logs, traces, and services pages. Done.
2. Host metrics ([[../tasks/00007-collect-host-and-service-metrics.md]]) and metric rollups ([[../tasks/00008-roll-up-metrics-to-1-minute-and-1.md]]). Done.
3. The journal and the index built from it ([[../tasks/00021-rebuild-the-index-from-a-journal.md]]).
4. Send mudro's traces to a local otelo, and measure the journal and the index ([[../tasks/00012-send-mudro-s-traces-to-a-local-s.md]]).
5. Auth ([[../tasks/00013-authenticate-with-passkeys-and-s.md]]), then run otelo on the droplet and turn off Better Stack.
6. `otelo guide` ([[../tasks/00011-print-a-debugging-guide-with-sin.md]]).

## Open questions

- Prior art to check first: OpenObserve (a single Rust binary with OTLP, but heavy for 1 GB).
