---
created: 2026-09-27T18:27:04Z
parent: ./design.md
---
# Telemetry

How otelo takes in logs, traces, and metrics, where it keeps them, and how a human or an agent reads them back. [[./design.md]] has the reasons for the whole product.

```mermaid
flowchart LR
  app[App with OTel SDK] -->|OTLP HTTP :4318| recv[OTLP receiver]
  app -->|OTLP gRPC :4317| recv
  machine[The machine and its services] --> host[Host collector]
  recv --> writer[Writer]
  host --> writer
  writer --> day[(telemetry/YYYY-MM-DD.sqlite)]
  day --> rollup[Rollups]
  rollup --> agg[(telemetry/metrics-rollup.sqlite)]
  day --> api[Query API]
  agg --> api
  api -->|HTTP| cli[otelo CLI]
  api -->|HTTP| ui[UI]
```

## Rules

- One writer task owns every write. Sources send batches to it over a bounded channel. When the channel is full, the source drops the batch and counts the drop, so a burst of telemetry never takes memory from the apps.
- SQLite in WAL mode. One file per UTC day for raw data. Retention deletes whole files. The defaults are 7 days of raw data, 14 days of 1-minute rollups, and 90 days of 1-hour rollups. [[../tasks/00008-roll-up-metrics-to-1-minute-and-1.md]] explains the rollups.
- A day file and the rollup file carry the version of their schema in `PRAGMA user_version`. When the writer starts, it sets a file of another version aside as `<file>.schema-<version>` and starts a new one. No file is migrated, so a renamed column takes the older rows out of every query. Retention deletes a day file it set aside with its day, and a rollup file it set aside once the file has not changed for the 90 days of the 1-hour rollups.
- A query that spans days attaches each day file. The query API caps a range at the retention, so the attach limit is never reached. A range of metrics can go back the 90 days of the 1-hour rollups.
- `service` is the OTel `service.name` resource attribute. The host collector sends no OTLP, so it names its services itself.

## Schema of a day file

```mermaid
erDiagram
  resources ||--o{ logs : has
  resources ||--o{ spans : has
  resources ||--o{ series : has
  series ||--o{ points : has
  attribute_keys ||--o{ attribute_values : has
  resources {
    int id PK
    int hash "xxh3 of service and attributes, unique"
    text service
    text attributes "JSON"
  }
  logs {
    int logged_at "unix nanos"
    int resource_id FK
    int severity "OTel severity number"
    text body "FTS5 index"
    blob trace_id
    blob span_id
    text attributes "JSON"
    text source "otlp"
  }
  spans {
    blob trace_id "indexed"
    blob span_id
    blob parent_span_id
    int resource_id FK
    text name
    int kind "OTel span kind number"
    int started_at "unix nanos"
    int duration_ns
    int status "OTel status code"
    text attributes "JSON"
    text events "JSON"
  }
  series {
    int id PK
    int hash "xxh3 of resource, name, kind, temporality, unit, labels, unique"
    int resource_id FK
    text name
    text kind "gauge, updown, counter, histogram"
    text temporality "cumulative or delta for a counter and a histogram, null for the rest"
    text unit
    text labels "JSON"
  }
  points {
    int series_id PK, FK
    int recorded_at PK "unix nanos"
    real value "the sum of a histogram point"
    text histogram "JSON buckets, null for the other kinds"
  }
  attribute_keys {
    text key_group PK "logs, spans, metrics, resource, or span_names"
    text key PK
    text value_type "JSON type of the values, or mixed"
    int count "records that have the key"
    int has_more_values "1 once the key has more values than attribute_values keeps"
  }
  attribute_values {
    text key_group PK
    text key PK
    text value PK "JSON"
    int count "records that have the value"
  }
```

The `attributes` and `labels` columns hold JSON objects. In Rust they are `Attributes`, a map of `AttributeValue`, which mirrors the `AnyValue` of OpenTelemetry: null, bool, int, double, string, array, and map. The JSON of the columns is the JSON of those types, so `json_extract` reads what the Rust code writes. A span event is a `SpanEvent` with its time, name, and attributes.

## OTLP receiver

The `otelo-otlp` crate serves OTLP over HTTP on `127.0.0.1:4318` (protobuf or JSON, gzip or not) and over gRPC on `127.0.0.1:4317`. `otelo serve --otlp-http` and `--otlp-grpc` move them. Both transports call one mapping to the rows:

- `service` is `service.name`, or `unknown_service` without one.
- The instrumentation scope becomes `otel.scope.name` and `otel.scope.version` on each record, and the status message of a span becomes `otel.status_description`, as the OTel spec maps them for formats without those fields. A log's `event_name` becomes `event.name`.
- A series has one of four kinds, which say how its points combine over time. A gauge is a `gauge`. A sum is a `counter` when it is monotonic and an `updown` when it is not. A histogram and an exponential histogram are a `histogram`. A `counter` and a `histogram` keep their temporality, `cumulative` or `delta`. [[../tasks/00008-roll-up-metrics-to-1-minute-and-1.md]] has the table of the kinds.
- Points are stored as they arrive. The increase of a cumulative counter is computed when it is read.
- A histogram point stores its sum as `points.value`. A point without buckets gets one bucket without bounds. An exponential histogram keeps its scale and its buckets, because the sender changes both as its values spread, and fixed bounds would not subtract.
- A point with the `NO_RECORDED_VALUE` flag is skipped. A query never fills a gap, so nothing needs a marker for the end of a series.
- The key of `points` is the series and the time, so a batch that is sent again overwrites its points.
- Summaries, a delta sum that is not monotonic, a sum or a histogram without a temporality, and a span without valid IDs are rejected. Span links, severity text, trace state, exemplars, and the start time of a point are not kept.
- Proto3 JSON leaves out a field at its default, and the decoder of `opentelemetry-proto` takes a point of an exponential histogram only with every field. The receiver fills the missing ones before it decodes.
- A rejected item, and every item of a request the full writer channel dropped, is counted in `partial_success`.
- A metric gets at most 1,000 series per day file, so one label that holds a user ID cannot fill the disk. The writer skips the points of a series past that. The receiver has answered by then, so the writer counts them in `otelo.telemetry.rejected_points`, next to `otelo.telemetry.dropped_batches`, and warns in its log.

## Host collector

The `otelo-host` crate reads the machine once when the daemon starts, and then every 15 seconds, on the wall-clock multiples of 15 seconds. It sends the points straight to the writer. It has no flag and no configuration. [[../tasks/00007-collect-host-and-service-metrics.md]] has the plan, and [[./platforms.md]] says where each number comes from.

OpenTelemetry treats a host as a resource of its own and gives host metrics no service name. The `service` column needs one, so otelo names itself, as an app that reports the metrics of its host does:

- The metrics of the machine and of otelo itself have the service `otelo`.
- The metrics of a service have the name of its systemd unit without `.service`, or its launchd label. A unit named like the `service.name` its app sends puts these numbers next to the telemetry of the app.
- Every resource carries `host.name`, `host.id`, `host.arch`, and `os.type`. A query finds the machine by `resource.host.name` and the `system.*` names, not by the service.

| Metric | Kind | Unit | Labels |
|---|---|---|---|
| `system.cpu.utilization` | gauge | `1` | `cpu.mode` on Linux: `user`, `nice`, `system`, `interrupt`, `iowait`, `steal`, `idle`. The shares add up to 1. |
| `system.cpu.load_average.1m`, `.5m`, `.15m` | gauge | `{thread}` | |
| `system.memory.usage` | updown | `By` | `system.memory.state`: `used`, `free`, and on Linux `cached` and `buffers` |
| `system.memory.limit` | updown | `By` | |
| `system.paging.usage` | updown | `By` | `system.paging.state`: `used`, `free` |
| `system.filesystem.usage` | updown | `By` | `system.device`, `system.filesystem.mountpoint`, `system.filesystem.type`, and `system.filesystem.state`: `used`, `free` |
| `system.network.io` | counter | `By` | `network.interface.name`, and `network.io.direction`: `receive`, `transmit` |
| `process.cpu.time` | counter | `s` | |
| `process.memory.usage` | updown | `By` | |
| `process.cgroup.memory.usage` | updown | `By` | |
| `otelo.storage.size` | updown | `By` | `otelo.storage.file`: `telemetry`, `rollup`, `state` |

- The kinds are the four of the model in [[../tasks/00008-roll-up-metrics-to-1-minute-and-1.md]]. A counter is cumulative, and a chart shows its rate.
- The names, units, and labels are the OpenTelemetry semantic conventions for system and process metrics, which are still in development and can change. Three names are not from there: the load averages have the names the OpenTelemetry Collector gives them, and `process.cgroup.memory.usage` and `otelo.storage.size` are otelo's.
- `iowait` is the time the CPUs sat idle waiting for the disk, and `steal` the time the hypervisor gave to another tenant. Without the two, a slow droplet at 30% CPU looks healthy.
- The `process.*` metrics cover a whole service, every process of its unit, and otelo's own process. `process.memory.usage` is the memory the processes hold themselves. `process.cgroup.memory.usage`, on Linux only, adds the page cache the unit filled, which is what `MemoryMax` and the OOM killer count.
- `otelo.storage.size` is the size of otelo's data. The storage backend reports it through `Storage::size()`, so the collector knows no file names. The SQLite backend adds up its files, and a WAL file counts with its database. The free space of the disk under them is in `system.filesystem.usage`.
- A filesystem counts once per device, under its shortest mount point. Filesystems without a disk, such as `tmpfs`, `overlay`, and `squashfs`, are left out. So are the loopback interface and every interface that has moved no bytes.
- A reading that fails, or a list of services that cannot be read, is a warning in the log. The collector sends what it has and tries again at the next tick.

## Rollups

The day files keep the raw points for 7 days. The writer also sums them up by the minute and by the hour in `telemetry/metrics-rollup.sqlite`, which keeps the minutes for 14 days and the hours for 90. [[../tasks/00008-roll-up-metrics-to-1-minute-and-1.md]] has the reasons.

- The file has the `resources` and `series` tables of a day file, and the tables `minutes` and `hours` in place of `points`, so a query reads the summaries as it reads the raw points.
- A row is one series and one minute or hour that has points, with the instant it starts in `start_at`: the count, minimum, maximum, sum, and last of the values, and for a `counter` how much it grew and over how many seconds, and for a `histogram` its merged buckets. A rolled up `counter` or `histogram` holds what each step added, so its temporality is `delta`.
- Once a minute the writer rolls up the minutes that ended 2 minutes ago or earlier, so a late batch is in them. An hour is rolled up from its 60 minutes once all of them are.
- The table `cursors` says up to where the minutes and the hours are rolled up. A daemon that was down starts there, or at the oldest day file when the table is empty, and rolls up an hour at a time with batches taken in between.
- A row is written with its key, the series and the start, so rolling a step up again gives the same row.
- The logic that sums points up by step is one type in `otelo-storage`. The query of the raw points and the rollups both use it, so a minute of summaries equals a minute of raw points.
- The rollup job reads the day files without a span. No request is its parent, so each read would show as a request of `otelo`.
- The writer sets a rollup file of another schema version aside, as it does a day file, and starts a new one from the day files it has.

## The daemon's own telemetry

`otelo serve` sends its own spans and logs over OTLP/HTTP to its own receiver, under the service `otelo` and with the host attributes of the host collector, so otelo can be tried and debugged on itself. `--own-telemetry off` keeps them on stderr only, and `--own-telemetry http://host:4318` sends them to another receiver. Every event also goes to stderr, where journald reads it.

- Each query API request gets a server span named after its route, such as `GET /api/logs`, with a child span for opening the reader and one `SELECT` span per SQLite statement, which carries the SQL and the rows it read. A 5xx response logs an error in the span.
- Nothing on the path from the OTLP receiver to the day files opens a span. A span there would make each export of the daemon's telemetry cause another export, forever. Events of the exporter's crates (`opentelemetry`, `reqwest`, `hyper`, `h2`, `tower`) stay on stderr for the same reason.
- On SIGTERM the daemon exports what it has queued before it stops its receivers, and what it logs after that reaches stderr only.

## Querying

The CLI and the UI use the same HTTP query API, which the `otelo-api` crate serves. The Rust types of the daemon make its OpenAPI spec, which `otelo openapi` prints. `packages/api` of the UI keeps it in `openapi.json` and generates its TypeScript types from it with `mise run api:generate`. A Rust test fails when `openapi.json` is behind the spec, and a test of `packages/api` fails when the types are behind `openapi.json`, so a change to the API reaches the UI's types. Every CLI command prints a table to a terminal and JSON otherwise, and `--json` and `--table` override that. Every query has a row limit. A cut result says so and names the flag that narrows it. Agents read what humans read, so the output stays small by default:

- `otelo logs` groups lines by message template first, with counts, and prints samples. `--raw` prints lines.
- `otelo spans` lists spans. `otelo traces` lists the traces that have a matching span, by root span, duration, and error flag. `otelo trace <id>` prints the span tree.
- `otelo metrics` lists the series. `otelo metric <name>` prints one metric at a step that fits the range: the count, minimum, average, maximum, and last value of each step for a `gauge` and an `updown`, the rate for a `counter`, and the percentiles for a `histogram`.
- A range of 6 hours at most reads the raw points. A range of 14 days at most reads the summaries by the minute, and a longer one those by the hour. A range that starts before the finer points are kept reads the next coarser ones. `--resolution raw`, `1m`, or `1h` picks one. A step of summaries is rounded up to whole minutes or hours, and the answer says which step and which points it read.
- The rate of a counter is its increase between two neighbouring points, divided by the time between them and not by the step, so a 30-second step over points a minute apart stays right. A cumulative value that goes down is a restart, and the increase counts from zero. The query also reads the 5 minutes before the range, so the first step has a point to count from.
- A histogram point keeps its buckets in `points.histogram` as JSON: `count`, `sum`, `min`, `max`, and either `bounds` and `counts` (one more than the bounds), or `scale`, `zero_count`, `positive`, and `negative` for an exponential histogram. The writer skips a point whose counts do not fit its bounds. `otelo metric` merges the points of each step into one set of bucket counts with p50, p90, and p99 estimates. A cumulative point counts as its increase over the point before, a drop in the counts is a restart, and the first cumulative point only sets where the counting starts. A step with points of different bounds keeps the newest bounds.
- Two exponential points always merge. Both go down to the lower scale, where each step joins neighbouring buckets in pairs. The API returns every distribution with explicit bounds, and joins the buckets of an exponential one until 64 are left. The percentiles are estimated before that. An explicit and an exponential point in one step do not merge, and the step keeps the newer one.
- `otelo services` lists the services that sent spans or logs, the most requests first, with their requests, errors, p50, p95, and p99 latency, logs, and error logs. `otelo service <name>` prints the same for one service and its requests by span name. A request is a span that enters the service: a root span, or a span of the server or the consumer kind. The percentiles come from buckets that each grow by 2%, so an estimate is off by 1% at most and the memory does not grow with the count of spans. `--buckets` adds the numbers of each step.
- `otelo calls <name>` prints the calls a service makes, by target and by what they do, the most time first. A call is a span of the client or the producer kind, or a span with `db.system.name` or `db.system` of any kind, since an in-process database such as SQLite may mark its spans internal. Its target comes from the OpenTelemetry attributes, the older names too: a database by system and `db.namespace`, a host by `server.address` or the host of `url.full`, with a port other than 80 or 443, an RPC service by `rpc.system` and `rpc.service`, a message destination by `messaging.system` and `messaging.destination.name`, and else `peer.service`. What a call does is `db.query.summary`, or the query with each string, number, and parameter such as `$1` or `:id` as `?` and a list of them as one `?`, for a database; the method and `url.template`, or the path with each id as `{id}`, for HTTP; and the span name for the rest. So `SELECT * FROM users WHERE id = 7` and `… id = 8` are one row, and two queries under one span name such as `SELECT` are two. Each target carries the span query terms that keep its calls, such as `db.system.name = "postgresql" db.namespace = "app"`.
- `otelo sql` runs a read-only query against the day files.

The logs page of the UI takes the same query, with completion from `/api/complete`, and a range of a preset or a custom `since` and `until`. It shows lines, or the message templates with their counts and samples. A line opens to its attributes and its resource, and a button next to a value adds `key = value` or `key != value` to the query. A template adds its fixed words to the query as `body ~ "…"` to show its lines. The page says which compared attributes have no index and can add one. Live mode reloads every 5 seconds. The query, the range, and the view live in the URL, so a link opens the same page.

The traces page takes a span query the same way. It lists the traces that have a matching span, by root span, span count, and duration, or the spans themselves. A span opens beside the list with its attributes, resource, and events, the same filter buttons, and a link to its trace. A trace opens in a modal over the list as a waterfall: each span under its parent, a bar where it runs on the time of the trace, and a fold for the children. A second tab shows the logs that carry the trace ID. The open trace lives in the URL of the list, so Back closes it. Each trace also has its own page, with the tab and the selected span in its URL, and the modal links to that page to share. A log line on the logs page links to its trace. The pages read what a span is from the OpenTelemetry conventions. An HTTP span shows its method, route, and status code on badges, in place of the words of the span name that say the same. A database span shows the icon of its system, such as PostgreSQL or Redis, and its query as the span has it, or its name when it has no query, with the first word on a badge when it is letters alone, such as `SELECT`. One function splits the text, so the traces pages, the calls of a service, and its modal show a query the same way. Every span starts with an icon of what it is, a globe, the icon of a database system or a plain database, or a plain span, so the names of a list line up. Each span shows its kind on a badge, and each service the icon of its language from `telemetry.sdk.language`, or of code when the language is unknown. The icons live in the `@otelo/icons` package. So the list of traces can do the same, `/api/traces` returns the kind, the attributes, and the resource of each root span.

The services page of the UI, its start page, lists the services of the range with the same numbers and a bar chart of the requests of each one, failed ones on top. A click on a column name sorts by it. A service opens its own page: the numbers of the range, then charts of the requests, the latency percentiles, the error rate, and the logs over the range, and its operations, the requests by span name and kind, each shown like a span, with the icon and badges of the attributes of its newest request. An operation opens in a modal over the page, with its numbers, its requests, latency, and error rate over the range from `/api/services/{name}/operation`, and its newest spans, each of which opens its trace. The open operation lives in the URL, so Back closes it. Its calls follow, from `/api/services/{name}/calls`: the time they take by target over the range, stacked for the 7 targets with the most time and the rest as one, their count over the range, and one table of what they do with the target of each, a database call as its query with the values taken out. A call opens in the same modal as an operation, from `/api/services/{name}/call`, which counts the calls of that target and summary and lists the newest 50 of them, and its target lives in the URL with it. The newest failed traces and the templates of the error logs close the page, with links to the traces and logs pages that show all of them. Dragging across a chart zooms the range to that stretch. The range and live mode live in the URL, and a link from one page to the other keeps the range.

## Query language

Every list takes one query, parsed by the `otelo-query` crate. The daemon spec is at `/api/openapi.json`.

```text
http.route = "/matches" OR (user.id = 7 AND http.response.status_code = 200)
level >= warn body ~ "payment failed"
root = true AND duration > 500ms AND NOT resource.host.name = "droplet"
```

- A name is a built-in field of the signal, `resource.<key>` for a resource attribute, `attr.<key>` for an attribute named like a built-in field, or else a record attribute. A key with other characters goes in backticks.
- Built-in fields. Logs: `service`, `level`, `body`, `trace_id`, `span_id`, `source`. Spans: `service`, `name`, `kind`, `status`, `error`, `duration`, `root`, `trace_id`, `span_id`. Metrics: `name`, `service`, `kind`, `unit`, and the labels as attributes.
- Operators: `= != < <= > >=`, `in (…)`, `~` (words in a log body through FTS5, a substring elsewhere), `has(key)`, `AND`, `OR`, `NOT`, and parentheses. Terms next to each other join with `AND`.
- A number also matches the same number sent as a string. `!=` and `NOT` keep the records that lack the attribute.

## Catalog and completion

The writer keeps the attribute keys of each signal and of the resources in `attribute_keys` in every day file, with their JSON type in `value_type` and their count. The `key_group` of a row says whose keys they are: `logs`, `spans`, `metrics` for the labels of the series, or `resource`. `attribute_values` keeps up to 200 values of each key, and `has_more_values` marks a key that has more. The group `span_names` holds the names of the spans under the key `name`. The writer only touches these tables for a new key or value and for the counts, once per transaction.

`otelo complete <signal> <query>` and `/api/complete` suggest the fields, operators, values, and keywords that fit at the cursor, from the catalog of the retention. `otelo attributes <signal>` lists the keys.

## Indexed attributes

`otelo index add logs user.id` stores the key in `state.sqlite` and hands the set to the writer. Within a second the writer creates an expression index on `json_extract(attributes, '$."user.id"')` in every day file, and in each new one. The index is named `logs_attribute_<hash>`, after its table and a hash of the key. `otelo index remove` drops it. The query compiler writes the same expression, so SQLite uses the index, also for an `OR` of indexed keys. A query on a key without an index still runs by reading the range, and the response names the key, so the CLI says which index would help. Only logs and spans take indexes; resources and series are small.

Until UI auth exists, the daemon listens on `127.0.0.1` only, and a laptop reaches it through an SSH tunnel.
