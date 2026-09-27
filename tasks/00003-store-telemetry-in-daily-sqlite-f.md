---
status: done
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
tags:
- feature
---
# Store telemetry in daily SQLite files

A crate that writes and reads the day files described in [[../docs/telemetry.md]].

- The schema from the doc, created when a day file opens. WAL mode.
- One writer task. Sources send batches over a bounded channel. A full channel drops the batch and adds to a drop counter that the daemon reports as a metric.
- Resources are deduplicated by a hash of their attributes.
- FTS5 on `logs.body`.
- Retention deletes day files older than the limit, 7 days by default. It runs at start and every hour.
- A reader attaches the day files a time range covers.
- Tests: write a batch of each kind, read it back across a day boundary, and delete a file past retention.
