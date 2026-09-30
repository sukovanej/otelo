---
status: in_review
created: 2026-09-30T12:33:34Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00008-roll-up-metrics-to-1-minute-and-1.md
- ./00015-group-the-series-of-a-metric-and.md
tags:
- feature
---
# Explore metrics in the UI

The metrics an app sends over OTLP can only be read with `otelo metrics` and `otelo metric`. This page reads them in the browser.

- A list of the metric names of the range, with the kind, the unit, and the count of series of each, from `/api/metrics`. It takes the metrics query with completion, as the logs and traces pages do.
- A metric opens as a chart. How it is drawn follows its kind in [[./00008-roll-up-metrics-to-1-minute-and-1.md]]: a `gauge` and an `updown` as lines, a `counter` as its rate, a `histogram` as p50, p90, and p99.
- A query narrows the series, and a group-by picker sets the `by` and `top` of [[./00015-group-the-series-of-a-metric-and.md]].
- The name, the query, the grouping, and the range live in the URL.
