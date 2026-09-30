---
status: backlog
created: 2026-09-30T12:33:34Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00007-collect-host-and-service-metrics.md
- ./00015-group-the-series-of-a-metric-and.md
tags:
- feature
---
# Show the machine on a host page

The host collector of [[./00007-collect-host-and-service-metrics.md]] stores the numbers of the machine, and only the CLI shows them. This page shows them in the browser with a fixed set of charts, built on `viz`.

- CPU utilization by mode, stacked, with `iowait` and `steal` in their own colors.
- The load averages.
- Memory by state, stacked against `system.memory.limit`, and swap.
- Each filesystem as a meter of used against total.
- Network bytes per second, received and sent, by interface.
- The services with the most CPU and the most memory, from the top N of [[./00015-group-the-series-of-a-metric-and.md]].
- otelo itself: its CPU, its memory, and `otelo.storage.size` by kind of file.

The page finds the machine by `resource.host.name` and the `system.*` names, not by a service. It takes the range picker and live mode of the other pages, and a drag across a chart zooms the range. The OTel unit of a series picks the `viz` unit: `By` is bytes, `s` is a duration, `1` is a ratio, and a counter is its unit per second.
