---
status: backlog
created: 2026-09-30T12:33:34Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00008-roll-up-metrics-to-1-minute-and-1.md
tags:
- feature
---
# Keep the exemplars of histogram points

An OTLP data point can carry exemplars: a few of the values it counted, each with the `traceId` and `spanId` of the request that produced it. The OTLP mapping drops them.

Kept, they let a latency chart link from a spike to one slow trace. The point stores them next to its buckets, the metric query returns the exemplars of each step, capped per step, and a chart marks them and opens the trace.

Nothing depends on this. Do it after the metrics explorer exists.
