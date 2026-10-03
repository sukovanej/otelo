---
status: backlog
created: 2026-10-03T22:33:34Z
tags:
- bug
---
# Keep otelo's RSS from growing with each UI query

otelo targets about 50 MB RSS, but on the droplet it grew from 16 MB to 184 MB in about two hours of UI use, against `MemoryMax=200M` in mudro's `otelo.service`. RSS rises with each UI page load and never comes back down.

## Cause

There is no leak in otelo's code. glibc gives each thread that allocates its own malloc arena: up to 8 on 1 vCPU, plus the main heap. A query runs on a tokio blocking thread. Each arena keeps the peak of the queries it served and almost never returns that memory to the OS.

Measured on v0.0.1 under Linux, with 100 replays of the service page (8 parallel requests each):

| Run | Before | After 100 loads | Idle |
|---|---|---|---|
| default glibc | 15.7 MB | 56.0 MB | 50.6 MB |
| `MALLOC_ARENA_MAX=1` | 15.2 MB | 27.9 MB | 27.9 MB |

- heaptrack shows a live heap of 2.5–2.9 MB between requests, with a peak of 13.9 MB inside a query. Only 0.9 MB was still allocated at exit.
- `/proc/<pid>/smaps` with default glibc: a 7.8 MB `[heap]` plus 9 anonymous regions of 3.5–4.3 MB each, about 35 MB in total. With one arena: a single 16.3 MB `[heap]`.
- On the droplet, a 5000-span `GET /api/spans` adds 15–20 MB. Prod has more data than the repro, so each query's peak is bigger, and the peak is kept once per arena.

## Work

- Limit glibc to one arena: call `mallopt(M_ARENA_MAX, 1)` at the start of `main`, before the tokio runtime starts any thread. Every install gets the fix without editing its unit. One arena costs nothing on 1 vCPU.
- Or switch the global allocator to jemalloc or mimalloc, which return freed pages on their own. Compare the RSS of both options with the same replay.
- Allocate less per query. A 5000-span response peaks at about 4 KB per span.
- The droplet can take `Environment=MALLOC_ARENA_MAX=1` in mudro's `deploy/otelo.service` right away.
