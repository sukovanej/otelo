---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
tags:
- feature
---
# Run the daemon with siner serve

`siner serve` starts the daemon. Later tasks add the receivers, the collectors, and the query API to it.

- An HTTP server on `127.0.0.1:7070`, with `--listen` to change it. `GET /health` answers 200.
- `--data <dir>`, default `/var/lib/siner` on Linux and `/usr/local/var/siner` on macOS. The daemon makes the directory when it is missing.
- SIGTERM and Ctrl-C stop the sources, flush the writer, and exit.
- The daemon logs to stderr, so journald keeps its log when systemd runs it.
