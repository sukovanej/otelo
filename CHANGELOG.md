# Changelog

All notable changes to otelo are in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/sukovanej/otelo/compare/v0.0.7...main)

### Changed

- `otelo serve` starts on a `telemetry.sqlite` of another storage version. It
  deletes the file and builds it again from the journal while it receives, so
  an update that changes the storage needs no stop and no `otelo reindex`.
  Until the indexer catches up, queries miss the newest hours. The daemon logs
  `caught up with the journal` when it has.

### Fixed

- While the indexer caught up with the journal, the metric rollups ran ahead of
  it. They marked minutes and hours as done before their points were indexed,
  and a chart over more than a few hours showed nothing for them. A rebuild of
  the storage lost the rollups of nearly every day it rebuilt. The rollups now
  stop at the last frame the indexer has read. The storage version goes to 4,
  so the daemon builds `telemetry.sqlite` again, with the rollups it lost.
- A thread of the daemon that panicked, such as the indexer, printed to stderr
  only, and nothing in otelo's own logs showed it. The panic is now an error
  log with its message, thread, file, line, and backtrace.

## [0.0.7](https://github.com/sukovanej/otelo/compare/v0.0.6...v0.0.7) - 2026-10-08

### Changed

- An arrow key in the query suggestions or the calendar, and a drag of a
  Select tag or a dashboard widget, re-run a tenth of the code they did. An
  arrow key in the calendar went from 43 re-runs to 4.
- The chart legend keeps its focus when the chart reloads.

### Fixed

- "Loading…" stayed on the logs and traces pages after a sort, a new query,
  or a reload.
- Removing a query tab to the right of the shown one showed the wrong tab.
- The sort header of the traces page lagged a click while the sorted list
  loaded, and a second click sorted from the old order.
- A second filter click during a load dropped the first term.
- An open suggestion list jumped back to the left on every keystroke.
- A menu removed while open kept its window listeners.
- Log out showed nothing while it ran and hid a failure. The button reads
  "Logging out…" and an alert shows the error.
- A dashboard widget whose query changed kept its old message with no sign
  of the load. The message dims while the widget loads.

## [0.0.6](https://github.com/sukovanej/otelo/compare/v0.0.5...v0.0.6) - 2026-10-08

The storage version goes up to 2. After the update, stop the daemon and run
`otelo reindex` before `otelo serve` starts again.

### Changed

- A query of one service reads only the rows of that service, through a new
  index on the service of a resource. It read every span or log line of the
  time range and checked the service of each. On 217k spans, counting the
  spans of a small service went from 36 ms to 1 ms.
- The query suggestions open only where an expression can start: in an empty
  input, or after a space, `(`, or `,`. They opened on every keystroke, click,
  and caret move. Taking a suggestion inserts its text and closes the list,
  and Arrow Down still opens it at the caret.
- `/api/openapi.json` is OpenAPI 3.2. Tools that read only 3.1 may refuse it.
- An otter is the logo and the favicon.

### Fixed

- A dashboard widget that changed from an unnamed metric to a named one kept
  "Pick a metric to see its numbers." while its title changed.
- A dashboard page copied its fetched definition a flush late, and Solid
  warned about it. A refetch now keeps a draft with edits and replaces one
  without.

## [0.0.5](https://github.com/sukovanej/otelo/compare/v0.0.4...v0.0.5) - 2026-10-06

### Added

- The `otelo` agent skill tells a coding agent how to investigate an app with
  the CLI: which command to run first, how to narrow a query, and how to go
  from a log line to its trace. `npx skills add sukovanej/otelo` installs it.
- `/api/spans`, `/api/traces`, and `/api/logs` return `next`, and take it back
  as `after` for the page after it. A page reads only its own rows. `otelo
  spans`, `otelo traces`, and `otelo logs --raw` follow `next` until they have
  `--limit` rows.
- A "wait for a blocking thread" span covers the time an API request waits
  for one of the two threads that run queries. It showed as a gap before
  `open reader`.
- `/api/complete` and `otelo complete` take `since`, `until`, and `context`.

### Changed

- Completion suggests only the fields and values that the rest of the query
  can match in the time range. `service = "caddy" and http.route = ` lists the
  routes of caddy, and a widget's filter lists the attributes of its metric.
  Operators follow the type of the attribute, and an `in` list leaves out the
  values it already has.
- The `limit` of `/api/spans`, `/api/traces`, and `/api/logs` goes up to 1000.
  The UI loads 200 more spans or log lines, or 50 more traces, as the end of a
  list scrolls into reach, where it read the whole list again with twice the
  limit.

### Fixed

- Reading four days of spans in pages of 10,000 grew the daemon's heap from
  46 to 155 MB in four minutes. Pages of at most 1000 rows keep it down.
- A chart's tooltip draws above the page and stays inside the window. A long
  series label in a widget's right half cut off its start.

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
