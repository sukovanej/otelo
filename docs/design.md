---
created: 2026-09-27T18:10:53Z
---
# Design

Siner deploys, runs, and observes the apps on one Linux or macOS server. It is one binary. The same binary is the daemon on the server and the CLI on a laptop or in CI.

The first user is mudro (conquer) on a DigitalOcean droplet: Ubuntu 24.04, 1 vCPU, 1 GB of memory. mudro, Caddy, and siner share that memory, so siner targets about 50 MB RSS.

## Decisions

- One server only. Multi-server is out of scope, and the README says so. The code can assume one SQLite and one clock.
- Native processes only: binaries, Node run directly, static files. No Docker for now.
- Siner does not own Caddy. Siner keeps stable paths (`<root>/apps/<app>/current/...`) and a fixed port per app, so the Caddyfile does not change on a deploy. `siner init` prints a Caddyfile snippet for the siner UI host.
- The service manager of the OS supervises the apps: systemd on Linux, launchd on macOS. Siner writes one unit or plist per app. [[./platforms.md]] lists what differs between the two.
- A CLI, not MCP. Humans and agents use the same commands. The CLI prints a table to a terminal and JSON otherwise; `--json` and `--table` override. Every query has a row limit and says when it truncated. `siner guide` prints how to debug with siner.
- App config is `siner.json` in the app repository, shipped inside the release tarball.
- No alerts for now. Health checks run and show in the UI and the timeline.

## Parts of the daemon

- Deploy API. It takes a tarball, unpacks it into `releases/<time>-<sha>/`, runs the drain hook, switches `current`, restarts the unit, waits for the health check, and rolls back when the check fails. It keeps the last N releases.
- Secrets. Encrypted at rest with age. Set with the CLI or the UI. The UI shows names, never values. The unit runs `siner exec <app>`, which gets the app's secrets from the daemon and starts the app with them in its environment. Secrets never touch the disk in plain text.
- OTLP receiver on 4317 (gRPC) and 4318 (HTTP) for logs, traces, and metrics.
- Service log source, so an app without OTel (Caddy, a backup script) still has logs: journald on Linux, the log files launchd writes on macOS.
- Host collector: CPU, memory, swap, load, disk, network, through the `sysinfo` crate on both platforms.
- Health checks: HTTP, TCP, command, and heartbeats (a job POSTs to siner; a missing POST is a failure).
- Timeline: deploys, restarts, OOM kills, health changes, and error logs in one stream.
- UI: a SolidJS SPA, built with Vite, in a pnpm workspace under `packages/`: `app` is the SPA, `ui` its components, its Tailwind v4 theme of colors and type, and its fonts, and `viz` the parts that show data: a chart of values over time, a table, a stat, a sparkline, and the panel that frames them. The pages build on `viz`, and so will custom dashboards, so its props are plain data and name units (`count`, `duration`, `ratio`, `rate`, `bytes`) and series colors (`series-1` to `series-8`, `error`, `p95`) by string, which a dashboard can keep as JSON. The UI ships IBM Plex Sans and IBM Plex Mono, so it looks the same on macOS, Windows, and Linux. A release binary embeds `packages/app/dist`, and the daemon serves it on the address of the API. Any path outside `/api` gets `index.html`.

## Code

The Rust workspace in `crates/` has one crate per part, and the `siner` binary puts them together:

| Crate | What it holds |
|---|---|
| `siner-query` | the query language: its parser and its completion |
| `siner-storage` | the storage interface: the model of the records, what the queries return, the channel to the writer, and the `Storage` and `Read` traits |
| `siner-storage-sqlite` | the SQLite backend: the day files, the writer, the queries, and the indexed attributes in the state file, `state.sqlite` |
| `siner-otlp` | the OTLP receiver over HTTP and gRPC |
| `siner-api` | the HTTP API, its errors, and its OpenAPI spec |
| `siner` | the binary: `siner serve`, the daemon's own telemetry, the web UI, and the CLI commands in `cli/` |

## Storage

SQLite. One state file (apps, deploys, secrets, checks). One telemetry file per day, so retention deletes old files. FTS5 for log search. Spans indexed by `trace_id`. Metrics in a narrow `(ts, name, labels_id, value)` table with 1-minute and 1-hour rollups.

## Deploy from GitHub

GitHub builds the artifact. The server does not build. The repository holds two secrets: `SINER_URL` and `SINER_TOKEN`.

```yaml
- run: siner deploy mudro target/droplet --sha ${{ github.sha }}
  env:
    SINER_URL: ${{ secrets.SINER_URL }}
    SINER_TOKEN: ${{ secrets.SINER_TOKEN }}
```

The URL is HTTPS through a hostname, because a token sent to a bare IP over HTTP can be read on the way. A deploy token is scoped to one app and can only deploy.

## UI and CLI auth

GitHub device flow. The siner project registers one GitHub OAuth App with the device flow on. Its client ID is public and compiled into the binary; the device flow needs no client secret and no callback URL, so an install needs no GitHub setup. `siner init` asks for the allowed GitHub usernames. The UI shows a code, the user enters it at github.com/login/device, and siner checks `/user` against the allowlist. `siner login` on a laptop uses the same flow.

Fallback: `siner ui-link`, run over SSH on the server, prints a one-time login URL.

## siner.json for mudro

```json
{
  "app": "mudro",
  "run": {
    "command": "./mudro serve --port 8080 --database {state}/mudro.sqlite --bank ./bank.sqlite",
    "secrets": ["MUDRO_GOOGLE_CLIENT_ID", "MUDRO_GOOGLE_CLIENT_SECRET", "MUDRO_FACEBOOK_*"]
  },
  "deploy": {
    "drain": { "command": "./mudro matches wait-idle", "timeout": "15m" },
    "keep": 5
  },
  "health": [
    { "name": "api", "http": "http://127.0.0.1:8080/languages", "every": "30s" }
  ],
  "heartbeats": [
    { "name": "backup", "expect_every": "25h" }
  ]
}
```

## What siner replaces in conquer

| Today | With siner |
|---|---|
| `DEPLOY_SSH_KEY`, rsync, `deploy/release.sh` | `SINER_URL`, `SINER_TOKEN`, `siner deploy` |
| `deploy/provision.sh` writes the units | `siner init`, then siner writes the units |
| `/etc/mudro/env` edited over SSH | `siner secret set` |
| Vector to Better Stack | journald tailer and host collector |
| OTLP traces to Better Stack | OTLP to `127.0.0.1:4318` |
| Better Stack heartbeat for the backup | siner heartbeat |

mudro changes: point `OTEL_EXPORTER_OTLP_ENDPOINT` at `http://127.0.0.1:4318`, add a `mudro matches wait-idle` command for the drain hook, and optionally turn on OTel logs so log lines carry trace IDs.

## Order

1. `init`, `deploy` with rollback, secrets, journald logs, host metrics, a basic UI. This removes Vector, `release.sh`, and the SSH deploy key.
2. OTLP traces and a waterfall view.
3. Health checks and heartbeats.
4. Timeline and the agent-facing CLI queries.
5. OTLP metrics and logs. Off-box backups.

## Open questions

- Rust or Go.
- Retention: 7 days of raw logs and traces, 90 days of metric rollups?
- Prior art to check first: OpenObserve (single Rust binary with OTLP; no deploys or secrets, and heavy for 1 GB).
