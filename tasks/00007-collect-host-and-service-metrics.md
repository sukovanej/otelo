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

Every 15 seconds, the machine's own numbers, read with the `sysinfo` crate on Linux and macOS, and named by the OTel semantic conventions:

- `system.cpu.utilization`, `system.cpu.load_average.1m`, `.5m`, `.15m`
- `system.memory.usage` and `system.paging.usage` (swap), by state
- `system.filesystem.usage` for each real mount, by state. On macOS, APFS volumes share their container's free space, so report the container once.
- `system.network.io` for each interface except loopback

For each service: `process.cpu.time` and `process.memory.usage` for the process tree under its main PID, with `service` set to the app. The PID comes from `systemctl show --property MainPID` on Linux and `launchctl print` on macOS. Which services to watch comes from a flag for now.

Tests: the mapping from a `sysinfo` snapshot to metric points is a function, tested with a built snapshot. The PID lookup parses recorded `systemctl` and `launchctl` output.
