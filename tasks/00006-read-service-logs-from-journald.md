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
# Read service logs from journald

Logs of apps that do not speak OTel, Caddy and the backup script among them, come from journald.

- Runs `journalctl --output=json --follow --after-cursor=<cursor>` as a child process. A child process keeps the binary free of libsystemd.
- Keeps the cursor in the data directory, so a restart neither loses nor repeats lines.
- `service` is `_SYSTEMD_UNIT` without `.service`. The severity comes from `PRIORITY`. The `source` column is `journald`.
- Records from systemd about a unit (`_PID=1`): starts, exits, and OOM kills. These keep a flag, so the timeline can find them later.
- Which units to read comes from a flag for now. Later the app config gives it.
- Linux only. The test reads a recorded `journalctl` JSON fixture.
