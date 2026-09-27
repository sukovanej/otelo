---
status: backlog
created: 2026-09-27T18:27:45Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00009-serve-the-query-api.md
tags:
- feature
---
# Print a debugging guide with siner guide

`siner guide` prints one page that tells an agent how to debug with siner: which command to run first, how to narrow a result, and how to go from a log line to its trace. It is the text a person would paste into an agent's prompt, so it stays under 100 lines. A test checks that every command the guide names exists.
