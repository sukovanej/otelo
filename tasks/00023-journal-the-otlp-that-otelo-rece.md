---
status: in_review
created: 2026-10-03T14:55:04Z
parent: ./00021-rebuild-the-index-from-a-journal.md
tags:
- feature
---
# Journal the OTLP that otelo receives

The journal keeps every OTLP export request that otelo accepts, as protobuf, for `journal_retention_days` (30 by default). [[./00024-index-from-the-journal-and-rebui.md]] builds the index from it. In this task the receiver still sends batches to the writer as now, and also appends to the journal.

## Layout

```
journal/
  logs/2026-10-03T14.seg          the open hour, appended to
  logs/2026-10-03T13.seg.zst      a closed hour
  traces/...
  metrics/...
```

- One segment per signal and UTC hour of receipt.
- A frame is little-endian: the length `u32`, a CRC-32C `u32` of the rest, `received_at` as Unix nanoseconds `i64`, then the protobuf of the `Export*ServiceRequest`.
- A request over OTLP/JSON or with gzip is decoded as now and encoded again as protobuf with `prost`.
- When the hour ends, a thread compresses the segment with zstd into `.seg.zst`, syncs it, and deletes the `.seg`. Start at level 3, and pick the level from the measurement in [[./00012-send-mudro-s-traces-to-a-local-s.md]].
- A crate of its own, `otelo-journal`: the frames, the segments, the compression, the retention, and a reader that streams the frames of a signal from a position. A position is the hour and the offset in the segment before compression.

## Durability

- The receiver answers after its frame is written and synced. The syncs are grouped: one `fsync` per signal every 200 ms at most covers every frame written since.
- At startup the open segment of each signal is read to its end. A short frame or a wrong CRC is a write that a crash cut off. The file is truncated before it, with a warning that names the segment and the bytes cut.
- A `.seg` of a past hour without its `.seg.zst` is a compression that a crash stopped. It is compressed again.

## The host collector

It builds an `ExportMetricsServiceRequest` in place of `Records`, and appends it as the receiver does. The daemon's own telemetry already arrives over OTLP.

## Retention and size

- Once an hour, the segments whose hour ended more than `journal_retention_days` ago are deleted.
- `StorageSize` gets `journal_bytes`, and `otelo.storage.file` gets `journal`.
- The [[../docs/telemetry.md]] doc and the Storage section of [[../docs/design.md]] change in the same commit.

## Tests

- Frames written and read back, from an open and from a compressed segment.
- A cut-off last frame and a frame with a wrong CRC are truncated at startup.
- A `.seg` without its `.seg.zst` is compressed at startup.
- A reader resumes from a position across the end of an hour and across the compression.
- Retention deletes the segments past the retention.
- A request over OTLP/JSON and the same request over protobuf give the same frame.

## Comments

### 2026-10-03T16:32:44Z by Milan Suk via claude-code

> Split in two crates at the user's request: otelo-journal holds the Journal trait, Position, and the sync ticket, and otelo-journal-files the filesystem and zstd backend. SIN-24 reads through the trait.
