# Changelog

All notable changes to otelo are in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/sukovanej/otelo/compare/v0.0.2...main)

## [0.0.2](https://github.com/sukovanej/otelo/compare/v0.0.1...v0.0.2) - 2026-10-04

### Added

- `otelo spans --sort` and `otelo traces --sort` list the `newest`, `oldest`,
  `longest`, or `shortest` first, and `/api/spans` and `/api/traces` take the
  same `sort`. A click on Time or Duration on the traces page sorts by it.
- The range picker has − and + beside its label. + takes Last hour to 2h, 3h,
  and so on, and − goes back, down to 5 minutes.

### Fixed

- The daemon runs at most two API queries at once. On Linux, glibc kept a
  memory arena for each of the eight queries a UI page sends, and otelo grew
  from 16 MB to 86 MB in an hour and a half of use.
- After a zoom on a chart, the selection no longer follows the pointer.

## [0.0.1](https://github.com/sukovanej/otelo/releases/tag/v0.0.1) - 2026-10-03

The first release.

### Added

- `otelo serve` receives OTLP logs, traces, and metrics on port 4317 (gRPC)
  and 4318 (HTTP). It writes each request to a journal before it answers, and
  indexes the journal into SQLite.
- Every 15 seconds the daemon records the CPU, load, memory, disks, and network
  of its host, and the CPU and memory of each systemd or launchd service.
- The daemon sends its own logs and traces to itself, under the service
  `otelo`.
- The daemon serves a web UI on the address of its API. It shows logs, traces
  and spans, metrics, and each service with its HTTP routes, database queries,
  and outgoing calls.
- One query language covers logs, spans, and metrics. The UI and
  `otelo complete` suggest what fits at the cursor, and `otelo index` adds an
  index on an attribute.
- The CLI reads the same API: `logs`, `spans`, `traces`, `trace`, `metrics`,
  `metric`, `services`, `service`, and `attributes`. It prints a table to a
  terminal and JSON otherwise.
- `otelo init` makes the password that the API asks for, as a cookie or a
  bearer token. `otelo serve --unsafe-no-auth` skips it and listens on
  loopback only.
- `otelo reindex` builds `telemetry.sqlite` again from the journal.
- `otelo update` installs the newest release, and `otelo update --canary` the
  newest build of `main`.
