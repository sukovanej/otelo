import { expect, test } from "vitest";

import type { LogCounts, MetricSeries, SeriesGroup, SpanGroups, SpanStats } from "@otelo/api";

import {
  combineMeasuredQueries,
  measureLogCounts,
  measureMetricSeries,
  measureSpanGroups,
} from "../src/dashboards/measure";

const START_AT = "2026-10-04T10:00:00Z";
const END_AT = "2026-10-04T10:02:00Z";
const MINUTE_NS = 60e9;

function toStats(count: number, errors: number, p95: number): SpanStats {
  return {
    count,
    errors,
    total_ns: count * p95,
    latency: count > 0 ? { p50: p95 / 2, p95, p99: p95 * 2 } : null,
  };
}

function toBuckets(stats: SpanStats[]) {
  return stats.map((spans, index) => ({
    start_at: new Date(Date.parse(START_AT) + index * 60_000).toISOString(),
    spans,
  }));
}

const SPAN_GROUPS: SpanGroups = {
  start_at: START_AT,
  end_at: END_AT,
  step_ns: MINUTE_NS,
  spans: toStats(6, 1, 30),
  buckets: toBuckets([toStats(2, 0, 10), toStats(4, 1, 40)]),
  groups: [
    {
      values: { service: "api" },
      name: "GET /",
      attributes: {},
      spans: toStats(5, 1, 20),
      buckets: toBuckets([toStats(2, 0, 10), toStats(3, 1, 30)]),
    },
    {
      values: { service: "worker" },
      name: "job",
      attributes: {},
      spans: toStats(1, 0, 90),
      buckets: toBuckets([toStats(0, 0, 0), toStats(1, 0, 90)]),
    },
  ],
  truncated: false,
  unindexed: [],
};

test("spans without grouping are one series of the whole range", () => {
  const measured = measureSpanGroups(SPAN_GROUPS, "error_rate", [], "Error rate");
  expect(measured.unit).toBe("ratio");
  expect(measured.groups).toEqual([
    { label: "Error rate", values: [0, 0.25], total: 1 / 6, color: "error" },
  ]);
  const rate = measureSpanGroups(SPAN_GROUPS, "rate", [], "Rate");
  expect(rate.groups[0]?.values).toEqual([2 / 60, 4 / 60]);
  expect(rate.groups[0]?.total).toBe(6 / 120);
});

test("groups of spans keep the order and the limit of the daemon", () => {
  const measured = measureSpanGroups(SPAN_GROUPS, "p95", ["service"], "P95");
  expect(measured.groups.map((group) => [group.label, group.total])).toEqual([
    ["api", 20],
    ["worker", 90],
  ]);
  expect(measured.truncated).toBe(false);
});

test("log counts keep the groups of the daemon", () => {
  const counts: LogCounts = {
    start_at: START_AT,
    end_at: END_AT,
    step_ns: MINUTE_NS,
    count: 3,
    buckets: [
      { start_at: START_AT, count: 1 },
      { start_at: "2026-10-04T10:01:00Z", count: 2 },
    ],
    groups: [
      {
        values: { level: "error" },
        count: 3,
        buckets: [
          { start_at: START_AT, count: 1 },
          { start_at: "2026-10-04T10:01:00Z", count: 2 },
        ],
      },
    ],
    truncated: false,
    unindexed: [],
  };
  expect(measureLogCounts(counts, [], "Logs").groups).toEqual([
    { label: "Logs", values: [1, 2], total: 3 },
  ]);
  expect(measureLogCounts(counts, ["level"], "Logs").groups[0]?.label).toBe("error");
});

function toMemoryGroup(service: string, values: number[]): SeriesGroup {
  return {
    key: { type: "series", service, attributes: {}, resource: {} },
    kind: "updown",
    unit: "By",
    buckets: values.map((value, index) => ({
      start_at: new Date(Date.parse(START_AT) + index * 60_000).toISOString(),
      count: 1,
      min: value,
      max: value,
      avg: value,
      last: value,
      change: { kind: "none" },
    })),
  };
}

test("the series of a metric add up without grouping", () => {
  const memory: MetricSeries = {
    name: "process.memory.usage",
    start_at: START_AT,
    end_at: END_AT,
    step_ns: MINUTE_NS,
    resolution: "raw",
    groups: [toMemoryGroup("api", [100, 200]), toMemoryGroup("worker", [50, 50])],
    truncated: false,
  };
  const measured = measureMetricSeries(memory, "avg", [], "Memory", "kept");
  expect(measured?.unit).toBe("bytes");
  expect(measured?.groups).toEqual([{ label: "Memory", values: [150, 250], total: 200 }]);
  expect(
    measureMetricSeries({ ...memory, groups: [] }, "avg", [], "Memory", "kept"),
  ).toBeUndefined();
});

test("the queries of a chart line up on the frame of the first", () => {
  const spans = measureSpanGroups(SPAN_GROUPS, "count", [], "Count");
  const later = {
    ...spans,
    frame: { ...spans.frame, bucketStartsMs: [Date.parse(END_AT) - 60_000] },
    groups: [{ label: "Later", values: [7], total: 7 }],
  };
  expect(combineMeasuredQueries([spans, later])?.series).toEqual([
    { label: "Count", values: [2, 4] },
    { label: "Later", values: [null, 7] },
  ]);
  expect(combineMeasuredQueries([])).toBeUndefined();
});

test("a chart leaves out the queries in another unit than the first", () => {
  const counts = measureSpanGroups(SPAN_GROUPS, "count", [], "Count");
  const latency = measureSpanGroups(SPAN_GROUPS, "p95", [], "P95");
  const chart = combineMeasuredQueries([counts, latency]);
  expect(chart?.series.map((series) => series.label)).toEqual(["Count"]);
  expect(chart?.otherUnitQueryCount).toBe(1);
});

test("a ranking leaves out the group that folds the others of a metric", () => {
  const folded: SeriesGroup = {
    ...toMemoryGroup("", [10, 10]),
    key: { type: "other", group_count: 2, series_count: 3 },
  };
  const memory: MetricSeries = {
    name: "process.memory.usage",
    start_at: START_AT,
    end_at: END_AT,
    step_ns: MINUTE_NS,
    resolution: "raw",
    groups: [toMemoryGroup("api", [100, 200]), folded],
    truncated: false,
  };
  const ranked = measureMetricSeries(memory, "avg", ["service"], "Memory", "dropped");
  expect(ranked?.groups.map((group) => group.label)).toEqual(["api"]);
  expect(ranked?.truncated).toBe(true);
  const charted = measureMetricSeries(memory, "avg", ["service"], "Memory", "kept");
  expect(charted?.groups).toHaveLength(2);
});
