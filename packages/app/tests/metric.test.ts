import { expect, test } from "vitest";

import type { MetricSeries, SeriesGroup, SeriesInfo } from "@otelo/api";

import {
  listGroupingOptions,
  summarizeMetricNames,
  toMetricCharts,
  toMetricFrame,
} from "../src/metrics/metric";

const HOST = { "host.name": "droplet" };

function toSeriesInfo(
  name: string,
  attributes: SeriesInfo["attributes"],
  service = "otelo",
): SeriesInfo {
  return { name, kind: "updown", unit: "By", service, attributes, resource: HOST };
}

function toMetricSeries(groups: SeriesGroup[], stepSeconds = 60): MetricSeries {
  return {
    name: "system.memory.usage",
    start_at: "2026-09-30T10:00:30Z",
    end_at: "2026-09-30T10:10:30Z",
    step_ns: stepSeconds * 1e9,
    resolution: "raw",
    groups,
    truncated: false,
  };
}

function toBucket(minute: number, avg: number, change: SeriesGroup["buckets"][number]["change"]) {
  const startAt = `2026-09-30T10:${String(minute).padStart(2, "0")}:00Z`;
  return { start_at: startAt, count: 1, min: avg, max: avg, avg, last: avg, change };
}

function toGaugeGroup(
  attributes: SeriesInfo["attributes"],
  averagesByMinute: number[][],
): SeriesGroup {
  return {
    kind: "updown",
    unit: "By",
    key: { type: "series", service: "otelo", attributes, resource: HOST },
    buckets: averagesByMinute.map(([minute = 0, avg = 0]) =>
      toBucket(minute, avg, { kind: "none" }),
    ),
  };
}

function toHistogramGroup(service: string): SeriesGroup {
  return {
    kind: "histogram",
    temporality: "delta",
    unit: "ms",
    key: { type: "series", service, attributes: {}, resource: HOST },
    buckets: [
      toBucket(0, 0, {
        kind: "distribution",
        bounds: [],
        count: 1,
        counts: [1],
        sum: 5,
        percentiles: { p50: 5, p90: 9, p99: 12 },
      }),
    ],
  };
}

test("summarizeMetricNames counts the series of each name", () => {
  const names = summarizeMetricNames([
    toSeriesInfo("system.memory.usage", { "system.memory.state": "used" }),
    toSeriesInfo("system.memory.usage", { "system.memory.state": "free" }),
    { ...toSeriesInfo("system.network.io", {}), kind: "counter", temporality: "cumulative" },
  ]);
  expect(names).toEqual([
    { name: "system.memory.usage", kinds: ["updown"], units: ["By"], seriesCount: 2 },
    { name: "system.network.io", kinds: ["counter"], units: ["By"], seriesCount: 1 },
  ]);
});

test("listGroupingOptions offers the attributes, the service, and the resource keys that differ", () => {
  const options = listGroupingOptions(
    [
      { ...toSeriesInfo("m", { state: "used", name: "a" }), resource: { "host.name": "a", x: 1 } },
      { ...toSeriesInfo("m", { state: "free" }), resource: { "host.name": "b", x: 1 } },
    ],
    ["resource.x", "gone"],
  );
  expect(options).toEqual([
    { value: "attr.name", label: "name", section: "Attributes" },
    { value: "state", label: "state", section: "Attributes" },
    { value: "gone", label: "gone", section: "Attributes" },
    { value: "service", label: "service", section: "Service" },
    { value: "resource.host.name", label: "resource.host.name", section: "Resource" },
    { value: "resource.x", label: "resource.x", section: "Resource" },
  ]);
});

test("toMetricFrame lays the steps from the one that holds the start of the range", () => {
  const frame = toMetricFrame(toMetricSeries([], 300));
  expect(frame.stepMs).toBe(300_000);
  expect(frame.bucketStartsMs.map((startMs) => new Date(startMs).toISOString())).toEqual([
    "2026-09-30T10:00:00.000Z",
    "2026-09-30T10:05:00.000Z",
    "2026-09-30T10:10:00.000Z",
  ]);
});

test("a series is a line of its averages that stays up to 5 minutes over empty steps", () => {
  const metric = toMetricSeries(
    [
      toGaugeGroup({ state: "used" }, [
        [0, 1],
        [2, 3],
      ]),
      toGaugeGroup({ state: "free" }, [[1, 2]]),
    ],
    60,
  );
  const [chart, ...otherCharts] = toMetricCharts(metric, toMetricFrame(metric), []);
  expect(otherCharts).toEqual([]);
  expect(chart?.unit).toBe("bytes");
  expect(chart?.series).toEqual([
    { label: "used", values: [1, 1, 3, 3, 3, 3, 3, null, null, null, null] },
    { label: "free", values: [null, 2, 2, 2, 2, 2, null, null, null, null, null] },
  ]);
});

test("a series is labeled by what tells it apart, and a group by its values", () => {
  const first = toGaugeGroup({ state: "used" }, [[0, 1]]);
  const second = {
    ...first,
    key: {
      type: "series",
      service: "otelo",
      attributes: { state: "used" },
      resource: { "host.name": "backup" },
    },
  } satisfies SeriesGroup;
  const metric = toMetricSeries([first, second]);
  expect(
    toMetricCharts(metric, toMetricFrame(metric), [])[0]?.series.map((series) => series.label),
  ).toEqual(["host.name=droplet", "host.name=backup"]);

  const grouped = toMetricSeries([
    { ...first, key: { type: "values", values: { state: "used" }, series_count: 2 } },
    { ...first, key: { type: "values", values: {}, series_count: 1 } },
    { ...first, key: { type: "other", group_count: 3, series_count: 5 } },
  ]);
  expect(
    toMetricCharts(grouped, toMetricFrame(grouped), ["state"])[0]?.series.map((series) => [
      series.label,
      series.color,
    ]),
  ).toEqual([
    ["used", undefined],
    ["–", undefined],
    ["3 other groups", "muted"],
  ]);
});

test("a counter is its rate, in bytes per second for bytes", () => {
  const metric = toMetricSeries([
    {
      kind: "counter",
      temporality: "cumulative",
      unit: "By",
      key: { type: "series", service: "otelo", attributes: {}, resource: HOST },
      buckets: [
        toBucket(0, 100, { kind: "none" }),
        toBucket(1, 160, { kind: "rate", per_second: 1 }),
      ],
    },
  ]);
  const [chart] = toMetricCharts(metric, toMetricFrame(metric), []);
  expect(chart?.unit).toBe("bytes-per-second");
  expect(chart?.series[0]?.values.slice(0, 3)).toEqual([null, 1, 1]);
});

test("a histogram draws its percentiles, one chart for each when it has several groups", () => {
  const lone = toMetricSeries([toHistogramGroup("api")]);
  const [loneChart, ...otherLoneCharts] = toMetricCharts(lone, toMetricFrame(lone), []);
  expect(otherLoneCharts).toEqual([]);
  expect(loneChart?.unit).toBe("duration");
  expect(loneChart?.series.map((series) => [series.label, series.values[0]])).toEqual([
    ["P50", 5e6],
    ["P90", 9e6],
    ["P99", 12e6],
  ]);

  const several = toMetricSeries([toHistogramGroup("api"), toHistogramGroup("web")]);
  const charts = toMetricCharts(several, toMetricFrame(several), []);
  expect(charts.map((chart) => chart.title)).toEqual([
    "P50 of each group",
    "P90 of each group",
    "P99 of each group",
  ]);
  expect(charts[2]?.series.map((series) => [series.label, series.values[0]])).toEqual([
    ["api", 12e6],
    ["web", 12e6],
  ]);
});

test("series of another kind or unit get a chart of their own", () => {
  const bytes = toGaugeGroup({}, [[0, 1]]);
  const metric = toMetricSeries([bytes, { ...bytes, unit: "MiBy" }]);
  expect(
    toMetricCharts(metric, toMetricFrame(metric), []).map((chart) => chart.description),
  ).toEqual(["updown in By", "updown in MiBy"]);
});
