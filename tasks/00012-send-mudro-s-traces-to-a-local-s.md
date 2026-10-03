---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00004-receive-otlp-over-http.md
- ./00009-serve-the-query-api.md
- ./00024-index-from-the-journal-and-rebui.md
tags:
- feature
---
# Send mudro's traces to a local otelo

The first real client. Run `otelo serve` on the laptop and `mise dev` in conquer with `OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318` and no Better Stack headers. Play a match. Then `otelo traces --service mudro` lists the requests and the match events, and `otelo trace <id>` shows the SQLite spans under one of them. Write down each thing in mudro's spans that otelo shows badly.

## Measure the journal and the index

Run otelo with the journal of [[./00021-rebuild-the-index-from-a-journal.md]] on mudro's traffic, a day of the droplet if it can, and write down:

- the bytes per day of the journal of each signal, before and after zstd, and the level that is worth its CPU
- the size of `telemetry.sqlite` per day of each signal
- how many rows a second the indexer writes, and how long a retention pass takes, with the FTS5 deletes
- how long `otelo reindex` takes for 7 days

The numbers say whether 30 days of journal fit the disk of the droplet, and whether the receiver must stay up during a reindex.
