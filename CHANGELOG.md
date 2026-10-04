# Changelog

All notable changes to otelo are in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/sukovanej/otelo/compare/v0.0.4...main)

## [0.0.4](https://github.com/sukovanej/otelo/compare/v0.0.3...v0.0.4) - 2026-10-04

### Added

- `--daemon` takes the name of a remote as well as an address. `otelo remote
  add prod https://otelo.example.com` keeps the name in
  `~/.config/otelo/remote.json`, and `otelo logs --daemon prod` reads that
  daemon. A value with / or : is an address, and anything else is a name.
- `otelo login` keeps the password of a daemon in the keyring of the OS, the
  macOS Keychain or the Secret Service on Linux, and `otelo logout` deletes
  it. Where the keyring is unavailable or has no password for the address, the
  CLI sends `OTELO_PASSWORD`. A 401 says which of the two to fix.

### Fixed

- A filter of alternatives, `service in (a, b, c)` or `service = a OR
  service = b`, kept only its first alternative inside the time range and the
  metric. The others matched records of any time and any metric, so a metric
  grouped by service also drew the other metrics of those services.
- A widget's action buttons no longer cut its title short. They take no width
  until the widget is hovered or holds focus, and on a touch screen they
  always show.

## [0.0.3](https://github.com/sukovanej/otelo/compare/v0.0.2...v0.0.3) - 2026-10-04

### Added

- Dashboards. A dashboard is a grid of 12 columns of widgets: time series,
  values, top lists, and notes. Widgets move and resize on the grid, and
  changes stay in the browser until Save. `state.sqlite` keeps the dashboards,
  and `/api/dashboards` and `otelo dashboard` list, show, create, replace, and
  delete them.
- `/api/spans/groups` takes `rank` and `order`, and `/api/metrics/{name}`
  takes `order`. `/api/logs/counts` is new and takes `order` too. A top list
  of the lowest P95 of routes shows the fastest routes.
- In the fields of a log line or a span, the service opens its page, trace_id
  opens the trace, and span_id and parent_span_id open the trace with that
  span selected.
- A list loads its next page when its end scrolls within 600px of the bottom
  of the page. The Show more button is gone.

### Fixed

- A chart redraws only what changed. Three series with gaps subscribed one
  effect to 45 sources, and a reload with the same data redrew every path.

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
