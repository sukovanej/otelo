---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00002-run-the-daemon-with-siner-serve.md
- ./00003-store-telemetry-in-daily-sqlite-f.md
tags:
- feature
---
# Collect host and service metrics

Every 15 seconds, the machine's own numbers, named by the OTel semantic conventions:

- `system.cpu.utilization`, `system.cpu.load_average.1m`, `.5m`, `.15m`
- `system.memory.usage` and `system.paging.usage` (swap), by state
- `system.filesystem.usage` for each real mount, by state
- `system.network.io` for each interface except `lo`
- For each systemd service: CPU time and memory from its cgroup v2 files, as `process.cpu.time` and `process.memory.usage` with `service` set to the unit

Linux only: `/proc`, `statfs`, and `/sys/fs/cgroup`. The tests read fixture copies of those files.
