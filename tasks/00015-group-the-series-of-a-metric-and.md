---
status: in_review
created: 2026-09-30T12:33:34Z
parent: ./00001-collect-and-query-telemetry.md
dependencies:
- ./00008-roll-up-metrics-to-1-minute-and-1.md
tags:
- feature
---
# Group the series of a metric and keep the top ones

`/api/metrics/{name}` returns each series on its own and stops at 20. A metric with several labels has far more, and nothing sorts them.

```text
http.server.request.duration  by method, route, status  ->  about 200 series
                              grouped by http.route     ->  one line per route
```

- A `by` parameter takes label names and `resource.<key>` names. Series with the same values of those become one.
- How they combine follows the kind of [[./00008-roll-up-metrics-to-1-minute-and-1.md]]: a `counter` adds the rates, an `updown` adds the values, a `gauge` takes the average, and a `histogram` merges the buckets.
- A `top` parameter keeps the N groups with the highest value over the range and returns the rest as one group named `other`. Without `by` each series is its own group, so `top` also answers which service uses the most memory.
- `otelo metric` takes `--by` and `--top`.
- The rollup tables of [[./00008-roll-up-metrics-to-1-minute-and-1.md]] answer the same query for a long range.

Tests: each kind grouped by one label, a group over two resources, and the top 3 with the rest as `other`.
