---
created: 2026-09-27T19:27:32Z
parent: ./design.md
---
# Platforms

The daemon runs on Linux and on macOS. The CLI runs anywhere, Windows included. [[./design.md]] has the rest of the product.

## What differs

| Part | Linux | macOS |
|---|---|---|
| What runs `otelo serve` | a systemd unit | a launchd daemon |
| Host metrics | `sysinfo` crate | `sysinfo` crate |
| Per-service CPU and memory | `sysinfo`, for the process tree under the PID from `systemctl show --property MainPID` | `sysinfo`, for the process tree under the PID from `launchctl print` |
| Logs of apps without OTel, if [[../tasks/00006-read-service-logs-from-journald.md]] stays | journald, read with `journalctl --output=json --follow` | the log files launchd writes, tailed |
| Root directory | `/var/lib/otelo` | `/usr/local/var/otelo` |

## One code path where it can

- Metrics use `sysinfo` on both platforms, so there is no metrics code per platform. cgroup v2 numbers on Linux are more exact and can come later.
- `#[cfg(target_os)]` appears only in the code that finds the PID of a service and, if [[../tasks/00006-read-service-logs-from-journald.md]] stays, in the service log source.

## Testing

- macOS: `mise check` runs every test natively.
- Linux: a Lima VM with Ubuntu 24.04, the same system as the droplet. Lima forwards the VM's ports to the Mac's `localhost`, so an otelo in the VM receives OTLP from apps on the Mac.
- CI, once it exists: an `ubuntu-24.04` and a `macos` runner, each running the whole suite.
