---
status: in_progress
created: 2026-10-03T22:33:34Z
tags:
- bug
pull_requests:
- https://github.com/sukovanej/otelo/pull/46
- https://github.com/sukovanej/otelo/pull/56
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

## What others do

Long-running multithreaded services on glibc commonly report a stable heap with RSS that keeps growing. There are three standard fixes:

- **jemalloc** (`tikv-jemallocator`). This is the most common choice in Rust servers. Redis and Firefox have used jemalloc for years for this reason. After a decay period (`dirty_decay_ms`), it returns unused pages to the OS. A crate's benchmark notes say RSS goes back to baseline after tokio bursts with jemalloc, and stays near the peak with glibc and mimalloc. The repository was archived in 2025, but Meta took it over again and released 5.3.1 in March 2026. Proxmox found the opposite once: jemalloc never released memory that their Rust library had allocated.
- **mimalloc**. rust-analyzer uses it: faster than jemalloc, slightly more memory, and a 3 MB smaller binary. One comparison found that mimalloc 2.2 used more memory than 2.1 for one workload.
- **glibc with limits**. Heroku has set `MALLOC_ARENA_MAX=2` by default for new Ruby apps since 2019. Some services also call `malloc_trim(0)` periodically or when idle. It goes through every arena and returns free pages to the OS.

Everyone says the result depends on the workload, and that allocating less does more than switching allocators.

Sources:

- [Heroku: Tuning glibc memory behavior](https://devcenter.heroku.com/articles/tuning-glibc-memory-behavior)
- [Heroku changelog: MALLOC_ARENA_MAX=2 default](https://devcenter.heroku.com/changelog-items/1683)
- [Mike Perham: Taming Rails memory bloat](https://mikeperham.com/2018/04/25/taming-rails-memory-bloat)
- [Meta: renewed commitment to jemalloc](https://engineering.fb.com/2026/03/02/data-infrastructure/investing-in-infrastructure-metas-renewed-commitment-to-jemalloc/)
- [Who even uses jemalloc in 2026](https://theconsensus.dev/p/2026/04/16/who-even-uses-jemalloc-anyway.html)
- [Proxmox: disable jemalloc RFC](https://lore.proxmox.com/all/20201210152338.19423-1-s.reiter@proxmox.com/t)
- [rust-analyzer allocator commits](https://git.joshthomas.dev/language-servers/rust-analyzer/commits/commit/eb99c35b3da43020df074b91243f6b51d792d116/crates/ra_prof)
- [jemalloc vs mimalloc vs tcmalloc for Rust](https://www.kunalganglani.com/blog/rust-allocator-jemalloc-mimalloc-tcmalloc)
- [vtcode allocator benchmark source](https://docs.rs/crate/vtcode/latest/source/src/cli/bench_allocator.rs)
- [FinBox: fixing memory retention with jemalloc](https://www.finbox.in/blog/beyond-python-heap-fixing-memory-retention-with-jemalloc)

## Work

1. Build three candidates, then replay 100 service page loads against each and compare peak RSS and RSS after a few idle minutes:
   - jemalloc as the global allocator
   - mimalloc as the global allocator
   - glibc with `mallopt(M_ARENA_MAX, 1)` at the start of `main`, before the tokio runtime starts any thread, plus `malloc_trim(0)` after a query
2. Ship the winner. jemalloc is the likeliest winner, because memory should come back after a burst. A C allocator has to build for all four release targets of cargo-dist.
3. Allocate less per query. A 5000-span response peaks at about 4 KB per span.

Until this lands, the droplet can take `Environment=MALLOC_ARENA_MAX=1` in mudro's `deploy/otelo.service`.

## Comments

### 2026-10-03T22:56:52Z by Milan Suk via claude-code

> The blocking pool is capped at 2 threads (#46): 33 MB instead of 50 MB after 100 page loads in the replay. The allocator comparison is still to do.
