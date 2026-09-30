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
| Root directory | `/var/lib/otelo` | `/usr/local/var/otelo` |
| `host.id` | `/etc/machine-id`, or else `/var/lib/dbus/machine-id` | the `IOPlatformUUID` line of `ioreg -rd1 -c IOPlatformExpertDevice` |
| CPU utilization | by mode, from the change of the first line of `/proc/stat` between two ticks | one total, from `sysinfo` |
| Memory | `free`, `cached`, and `buffers` from `/proc/meminfo`, and `used` as the rest of the total | `used` and `free` from `sysinfo` |
| Load, swap, filesystems, network | `sysinfo` | `sysinfo`. The volumes of one APFS container count once: they are the APFS filesystems with the same size and the same free space |
| Which services exist | every `*.service` directory under `/sys/fs/cgroup/system.slice` and the slices in it, whose `cgroup.events` says `populated 1` | every row of `launchctl list` with a PID, except the `com.apple.*` labels of macOS and the `application.*` labels of apps a user opened |
| CPU and memory of a service | `usage_usec` of `cpu.stat`, `anon` of `memory.stat`, and `memory.current` of its cgroup | `sysinfo`, for the process tree under the PID |
| CPU and memory of otelo | `sysinfo`, for its own PID | `sysinfo`, for its own PID |

## Where the code differs

`sysinfo` reads what it gives equally well on both platforms. Linux reads `/proc` and cgroup v2 where they give more:

- `/proc/stat` splits the CPU time by mode. `sysinfo` gives one total, which hides `iowait` and `steal`.
- `/proc/meminfo` has `cached` and `buffers`. `cached` is `Cached` plus `SReclaimable`, as the OpenTelemetry Collector counts it.
- The kernel counts the CPU time of a cgroup itself, so the number of a service never drops when one of its processes exits, and nothing walks a process tree. A service needs no entry in a flag or a file, because its directory is there while it runs.

macOS has no cgroups. There the collector adds up the CPU time each live process of a tree gained since the last tick and keeps a running total for the service. A plain sum over the tree would drop when a child exits, and a rollup reads a drop as a restart of the counter.

The readers pick their source with `cfg!(target_os)`, a plain condition, so every reader compiles on every platform and the tests of the Linux files run on a Mac. The readers fill one `Snapshot` type, and the mapping from a snapshot to metric points is the same code on both platforms.

`sysinfo` gives the name of a volume on macOS and not its device, so the collector cannot read the container of an APFS volume from a name such as `disk3s5`. The volumes of one container all report the size and the free space of the container, and that is how the mapping finds them.

A daemon that does not run as root sees the launchd jobs of its user, and the CPU and memory of that user's processes.

Service metrics on Linux need cgroup v2, which Ubuntu 24.04 mounts by default. On a machine without it the collector says so once in its log and sends the rest.

## Testing

- macOS: `mise check` runs every test natively.
- Linux: a Lima VM with Ubuntu 24.04, the same system as the droplet. Lima forwards the VM's ports to the Mac's `localhost`, so an otelo in the VM receives OTLP from apps on the Mac.
- CI, once it exists: an `ubuntu-24.04` and a `macos` runner, each running the whole suite.
