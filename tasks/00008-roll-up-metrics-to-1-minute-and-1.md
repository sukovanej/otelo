---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00003-store-telemetry-in-daily-sqlite-f.md
tags:
- feature
---
# Roll up metrics to 1-minute and 1-hour points

Raw points stay in the day files for 7 days. A rollup job writes 1-minute and 1-hour points into `metrics-rollup.sqlite`, kept 90 days: min, max, sum, and count for gauges and sums, merged buckets for histograms. A metrics query past the raw retention, or with a step of a minute or more, reads the rollups.
