---
name: otelo
description: Telemetry investigation with the otelo CLI, over the logs, traces, spans, and metrics that otelo keeps for the apps on a server. Invoke when the user asks why an app failed, errored, got slow, or ran short of memory, CPU, or disk, or what it did at some time; when the user hands over a trace ID, a log line, a route, or a service to look into; and before running `otelo logs`, `spans`, `traces`, `trace`, `services`, `service`, `metrics`, or `metric`.
---

# Investigating with otelo

otelo keeps the OpenTelemetry logs, traces, and metrics of the apps on one server, and the
CPU, memory, disks, and network of the machine and of each service it runs. The `otelo` CLI
queries the daemon over HTTP. Every command in this skill reads, and none changes the daemon.

An investigation is a chain of evidence. Each command names a service, a time window, a route,
a log template, or a trace, and the next command narrows to it. The answer to the user cites
that evidence: trace IDs, UTC timestamps, counts, and latencies, each with the command that
printed it. Say what the telemetry does not show, too, such as a service that sent no spans.

## Which daemon

- `--daemon` takes an address or the name of a remote. Without it the CLI reads `OTELO_URL`,
  or else `http://127.0.0.1:7070`.
- `otelo remote list` prints the names, such as `mudro`. When the user says "production" or
  names a server, pick its remote; ask when two could fit.
- The CLI sends the password that the keyring keeps for the address, or else
  `OTELO_PASSWORD`. On `Error: no password`, ask the user to run
  `otelo login --daemon <name>`, which reads it from their terminal. The user types the
  password, and it stays out of your commands.
- Run the `otelo` on the PATH. A freshly built `target/debug/otelo` is a new program to the
  macOS Keychain, so a remote query from it makes the user approve every call.

## Output

The CLI prints JSON when stdout is not a terminal, so you get JSON unless you pass `--table`.
A table is a fraction of the size of the JSON and suits a first read of a list. JSON carries
what a table leaves out: every attribute, the resource, the events of a span. Read the list
with `--table`, then fetch the record you need with `--json`.

Notes go to stderr:

- `More groups match; narrow them with the query or --since, or raise --limit.` and its
  variants mean the result was cut at the limit. Narrow the query or the range before you
  raise `--limit`.
- `<key> has no index, so the query read every record in the range` means the answer is
  complete and was slower to get. Adding the index with `otelo index add` changes the daemon,
  so leave it to the user.

In JSON the same facts are `truncated`, `next`, and `unindexed`. A log grouping with
`partial: true` counted only the newest lines of a range that had more than it reads; shorten
the range for exact counts.

Times print in UTC. JSON durations are nanoseconds, in `duration_ns` and every `*_ns`. A span's
`kind` in JSON is 1 internal, 2 server, 3 client, 4 producer, 5 consumer, and its `status` is
0 unset, 1 ok, 2 error.

## Range and limits

- `--since` and `--until` take a duration before now, such as `30m`, `6h`, or `2d`, or an
  RFC 3339 timestamp. The range defaults to the last hour. `otelo trace` defaults to the whole
  retention.
- Each signal keeps 7 days by default, and the daemon caps a range at the retention.
- Fit the range to the symptom. "This morning around 9" is
  `--since 2026-10-06T06:30:00Z --until 2026-10-06T08:00:00Z` in UTC, and a wider range only
  dilutes the counts and the percentiles.
- `--limit` caps the rows. The daemon answers at most 1000 rows a page, and the CLI fetches
  pages until it has `--limit`.

## Steps

Start where the user's clue is. A trace ID starts at step 4, and a log line at step 3.

1. **Find the service.** `otelo services --since <range> --table` lists each service with its
   requests, errors, p50, p95, and p99 latency, logs, and error logs. A request is a server
   span with `http.request.method`, so a worker or a cron job shows 0 requests and may still
   have logs. Done when you know which services the symptom touches.
2. **Find when.** `otelo service <name> --since <range> --buckets --table` prints the same
   numbers per step, then the HTTP routes and the database queries of the service. `--step 1m`
   sets the step. Done when you have the window where errors, latency, or log volume changed,
   and the routes or queries that carry the change. Use that window as the range from here on.
3. **Read the logs.** `otelo logs 'service = <name> level >= warn' --table` groups the lines by
   message template, with a count and the last time of each. For each template that fits the
   symptom, print its lines with
   `otelo logs 'service = <name> body ~ "<fixed words of the template>"' --raw --table`. The
   TRACE column links a line to its trace. Done when every template of the window that fits is
   read, and the trace IDs of its lines are noted.
4. **Read the traces.** `otelo traces 'service = <name> error = true' --table` lists the failed
   ones, and `otelo traces 'service = <name>' --sort longest --table` the slow ones.
   `otelo trace <id> --table` prints the span tree with the offset and duration of each span.
   `otelo trace <id> --json` adds the attributes, the events with their exception messages,
   and the logs of the trace. Done when you can name the span that failed or took the time,
   and why, from its attributes, events, and logs.
5. **Read the metrics** when the cause may lie outside the app: CPU, memory, disk, network, or
   a neighbour service. The recipes below have the names. Done when the machine is ruled in or
   out for the window.

## Query language

Every list takes one query after its flags. Quote it in single quotes for the shell.

```text
service = mudro level >= warn
http.route = "/matches" AND http.response.status_code >= 500
root = true AND duration > 500ms AND NOT resource.host.name = "droplet"
body ~ "payment failed" OR body ~ timeout
service in (mudro, caddy) kind = server
```

- Terms side by side join with `AND`. The operators are `= != < <= > >=`, `in (a, b)`, `~`,
  `has(key)`, `AND`, `OR`, `NOT`, and parentheses.
- `~` matches words of a log body through full-text search, and a substring in every other
  field.
- `!=` and `NOT` also keep the records that lack the attribute. Add `has(key)` to keep only
  those that have it.
- A number also matches the same number sent as a string. A duration takes `ns`, `us`, `ms`,
  `s`, `m`, or `h`.
- Built-in fields of logs: `service`, `level`, `body`, `trace_id`, `span_id`. Of spans:
  `service`, `name`, `kind`, `status`, `error`, `duration`, `root`, `trace_id`, `span_id`. Of
  metrics: `name`, `service`, `kind`, `unit`.
- `level` is `trace`, `debug`, `info`, `warn`, `error`, or `fatal`. `kind` is `internal`,
  `server`, `client`, `producer`, or `consumer`. `status` is `unset`, `ok`, or `error`.
- Every other name is an attribute of the record, such as `http.route` or `user.id`.
  `resource.<key>` reads the resource, such as `resource.service.version`, and `attr.<key>`
  an attribute named like a built-in field. A key with other characters goes in backticks.
- A key that no record has matches nothing, without an error. Look keys up first:
  `otelo attributes spans --table` lists the keys of a signal with their types and counts,
  and `otelo complete spans 'http.route = ' --table` lists the values seen for one.

## Recipes

- **Log line to trace.** `otelo logs '<query>' --raw --json` gives the `trace_id` of each
  line, and `otelo trace <id>` its spans.
- **Trace to logs.** `otelo trace <id> --json` has the logs of the trace, and
  `otelo logs 'trace_id = <id>' --raw --table` lists them alone.
- **Slowest routes.**
  `otelo spans 'service = <name> kind = server' --by http.request.method,http.route --rank p95 --table`.
  `--rank` takes `time`, the default, `count`, `errors`, `error_rate`, `p50`, `p95`, or `p99`,
  and `--order lowest` puts the lowest first. Spans that lack a `--by` key land in one group
  shown as `-`, so filter to the spans that have it.
- **Failing routes.** Group by `http.route,http.response.status_code` with
  `--rank error_rate`.
- **Slowest requests.** `otelo spans 'service = <name> kind = server' --sort longest --limit 10 --table`.
- **Database time.**
  `otelo spans 'service = <name> has(db.system.name)' --by db.query.text --rank time --table`.
  An app that writes values into the SQL text gets one group per value, which spreads one
  slow query over many small groups.
- **Memory.** `otelo metric process.memory.usage --top 5 --table` names the 5 services that
  hold the most. On Linux, `process.cgroup.memory.usage` adds the page cache of the unit,
  which is what `MemoryMax` and the OOM killer count. `system.memory.usage --by
  system.memory.state` and `system.paging.usage` show the whole machine.
- **CPU.** `otelo metric process.cpu.time --top 5 --table` prints CPU seconds per second for
  each service, so 1 is one full core. `otelo metric system.cpu.utilization --by cpu.mode
  --table` splits the machine's CPU on Linux. High `steal` means the hypervisor gave the time
  to another tenant, and high `iowait` means the CPUs sat waiting for the disk; either makes
  a droplet slow at 30% CPU. `system.cpu.load_average.1m` is the load.
- **Disk and network.** `otelo metric system.filesystem.usage --by
  system.filesystem.mountpoint,system.filesystem.state --table`, and `otelo metric
  system.network.io --by network.interface.name,network.io.direction --table`.
- **An app's own metrics.** `otelo metrics 'service = <name>' --table` lists its series, and
  `otelo metric <name> --by <key> --table` prints one. A gauge prints min, average, max, and
  last per step, a counter its rate, and a histogram p50, p90, and p99.

## Where telemetry comes from

- The metrics of the machine and of otelo itself are under the service `otelo`. The metrics
  of a service the OS runs are under the name of its systemd unit without `.service`, or its
  launchd label. `resource.host.name` picks the machine.
- otelo traces its own query API. Each request is a span named after its route, such as
  `GET /api/logs`, with one `SELECT` span per SQLite statement. Look there when otelo itself
  answers slowly.
- otelo shows only what the apps send. A service missing from `otelo services` sent no spans
  or logs in the range, which is a finding of its own: the app may be down, or its exporter
  may point elsewhere.
