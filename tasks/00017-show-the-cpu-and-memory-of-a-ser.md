---
status: in_progress
created: 2026-09-30T12:33:34Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00007-collect-host-and-service-metrics.md
tags:
- feature
---
# Show the CPU and memory of a service on its page

The service page shows requests, latency, errors, and calls, all from spans. It does not show what the process costs the machine. The host collector of [[./00007-collect-host-and-service-metrics.md]] stores `process.cpu.time`, `process.memory.usage`, and on Linux `process.cgroup.memory.usage` under the name of the service's unit.

Add a resources section to the page: the CPU of the service as a share of one core over the range, and its memory, with the memory of its cgroup next to it where that exists. The charts share the range and the zoom of the rest of the page.

A service whose unit has another name than its `service.name` has no such series. The section then says so and names the unit metrics it looked for.
