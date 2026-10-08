import { expect, test } from "vitest";

import type { Api, MetricSeries, SeriesGroup } from "@otelo/api";

import { getServiceResources, toResourceCharts } from "../src/services/resources";
import { createFakeApi } from "./fake-api";

const SERVICE_KEY: SeriesGroup["key"] = {
  type: "values",
  values: { service: "mudro" },
  series_count: 1,
};

type Bucket = SeriesGroup["buckets"][number];

function toMetricSeries(name: string, groups: SeriesGroup[]): MetricSeries {
  return {
    name,
    start_at: "2026-09-30T10:00:00Z",
    end_at: "2026-09-30T10:03:00Z",
    step_ns: 60e9,
    resolution: "raw",
    groups,
    truncated: false,
  };
}

function toBucket(minute: number, avg: number, change: Bucket["change"]): Bucket {
  const startAt = `2026-09-30T10:0${minute}:00Z`;
  return { start_at: startAt, count: 4, min: avg, max: avg, avg, last: avg, change };
}

function toBytesGroup(averagesByMinute: number[][]): SeriesGroup {
  return {
    kind: "updown",
    unit: "By",
    key: SERVICE_KEY,
    buckets: averagesByMinute.map(([minute = 0, avg = 0]) =>
      toBucket(minute, avg, { kind: "none" }),
    ),
  };
}

const cpuTimeGroup: SeriesGroup = {
  kind: "counter",
  temporality: "cumulative",
  unit: "s",
  key: SERVICE_KEY,
  buckets: [
    toBucket(0, 120, { kind: "rate", per_second: 0.25 }),
    toBucket(1, 135, { kind: "rate", per_second: 0.5 }),
    toBucket(2, 165, { kind: "rate", per_second: 1.5 }),
  ],
};

const memoryGroup = toBytesGroup([
  [0, 100e6],
  [1, 110e6],
  [2, 120e6],
]);

const cgroupMemoryGroup = toBytesGroup([
  [0, 300e6],
  [2, 320e6],
]);

test("the CPU is the rate of the CPU time, which is a share of one core", () => {
  const charts = toResourceCharts({
    service: "mudro",
    cpuTime: toMetricSeries("process.cpu.time", [cpuTimeGroup]),
    memory: toMetricSeries("process.memory.usage", [memoryGroup]),
    cgroupMemory: toMetricSeries("process.cgroup.memory.usage", [cgroupMemoryGroup]),
  });
  expect(charts.state).toBe("shown");
  if (charts.state !== "shown") return;
  expect(charts.frame.bucketStartsMs).toHaveLength(3);
  expect(charts.cpuSeries).toEqual([{ label: "CPU", values: [0.25, 0.5, 1.5] }]);
  expect(charts.memorySeries).toEqual([
    { label: "Processes", values: [100e6, 110e6, 120e6] },
    { label: "Cgroup", values: [300e6, 300e6, 320e6] },
  ]);
});

test("the memory has no cgroup line where the host has no cgroups", () => {
  const charts = toResourceCharts({
    service: "mudro",
    cpuTime: toMetricSeries("process.cpu.time", [cpuTimeGroup]),
    memory: toMetricSeries("process.memory.usage", [memoryGroup]),
    cgroupMemory: toMetricSeries("process.cgroup.memory.usage", []),
  });
  expect(charts.state).toBe("shown");
  if (charts.state !== "shown") return;
  expect(charts.memorySeries.map((series) => series.label)).toEqual(["Processes"]);
});

test("a service without the series of a unit has none to show", () => {
  const charts = toResourceCharts({
    service: "mudro",
    cpuTime: toMetricSeries("process.cpu.time", []),
    memory: toMetricSeries("process.memory.usage", []),
    cgroupMemory: toMetricSeries("process.cgroup.memory.usage", []),
  });
  expect(charts).toEqual({ state: "missing" });
});

test("the series come from the host collector, not from the SDK of the app", async () => {
  const requests: MetricSeriesRequest[] = [];
  const api = createFakeApi({
    getMetricSeries: (name, query) => {
      requests.push({ name, query });
      return Promise.resolve(toMetricSeries(name, []));
    },
  });

  const resources = await getServiceResources(
    api,
    { service: 'say "hi"', since: "1h", until: "" },
    new AbortController().signal,
  );

  expect(resources.service).toBe('say "hi"');
  expect(resources.cgroupMemory.name).toBe("process.cgroup.memory.usage");
  const hostCollectorQuery = {
    q: 'service = "say \\"hi\\"" NOT has(resource.telemetry.sdk.name)',
    since: "1h",
    until: "",
    by: "service",
  };
  expect(requests).toEqual([
    { name: "process.cpu.time", query: hostCollectorQuery },
    { name: "process.memory.usage", query: hostCollectorQuery },
    { name: "process.cgroup.memory.usage", query: hostCollectorQuery },
  ]);
});

interface MetricSeriesRequest {
  readonly name: string;
  readonly query: Parameters<Api["getMetricSeries"]>[1];
}
