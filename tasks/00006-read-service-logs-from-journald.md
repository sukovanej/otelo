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
# Read the logs of apps without OTel

Logs of apps that do not speak OTel, Caddy and the backup script among them, come from the service manager. [[../docs/platforms.md]] has the table. One interface, the service log source, sends log records and service events to the writer. It has two implementations.

## Linux: journald

- Runs `journalctl --output=json --follow --after-cursor=<cursor>` as a child process. A child process keeps the binary free of libsystemd.
- Keeps the cursor in the data directory, so a restart neither loses nor repeats lines.
- `service` is `_SYSTEMD_UNIT` without `.service`. The severity comes from `PRIORITY`.
- The records systemd writes about a unit (`_PID=1`) become service events: start, exit with its code, restart, and OOM kill.

## macOS: launchd log files

- launchd writes each app's stdout and stderr to `<root>/logs/<app>.log`. Otelo tails the file and keeps its read offset in the data directory.
- When a file passes 10 MB, otelo truncates it after reading to the end. launchd opens the file in append mode, so the next write lands at the new end.
- Every line is `info`, except a line that starts with `error`, `warn`, or `debug`, which takes that level.
- Otelo polls `launchctl print system/otelo.<app>` every 5 seconds. A new PID is a start. A change of the last exit code is an exit.

## Both

- The `source` column is `service`.
- Which services to read comes from a flag for now. Later the app config gives it.
- Tests: the Linux parser reads a recorded `journalctl` JSON fixture, and the `launchctl print` parser reads a recorded output. These run on any machine. A macOS integration test starts an agent in the user domain (`gui/<uid>`) and reads its log and its events. A Linux one does the same with `systemd-run --user`.
