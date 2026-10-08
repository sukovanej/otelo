import type { Api, MetricSeries } from "@otelo/api";
import type { TimeFrame, TimeSeries } from "@otelo/viz";

import { readAveragesOrRates, toMetricFrame } from "../metrics/metric";
import { quoteString } from "../query";

export const RESOURCE_METRIC_NAMES = {
  cpuTime: "process.cpu.time",
  memory: "process.memory.usage",
  cgroupMemory: "process.cgroup.memory.usage",
} as const;

// An app can send process metrics of its own from its SDK, and they would add
// up with the ones the host collector reads.
const HOST_COLLECTOR_TERM = "NOT has(resource.telemetry.sdk.name)";

export interface ServiceResources {
  readonly service: string;
  readonly cpuTime: MetricSeries;
  readonly memory: MetricSeries;
  readonly cgroupMemory: MetricSeries;
}

export type ResourceCharts = MissingResourceCharts | ShownResourceCharts;

interface MissingResourceCharts {
  readonly state: "missing";
}

interface ShownResourceCharts {
  readonly state: "shown";
  readonly frame: TimeFrame;
  readonly cpuSeries: ReadonlyArray<TimeSeries>;
  readonly memorySeries: ReadonlyArray<TimeSeries>;
}

interface ServiceResourcesQuery {
  readonly service: string;
  readonly since: string;
  readonly until: string;
}

export async function getServiceResources(
  api: Api,
  query: ServiceResourcesQuery,
  signal: AbortSignal,
): Promise<ServiceResources> {
  const metricQuery = {
    q: `service = ${quoteString(query.service)} ${HOST_COLLECTOR_TERM}`,
    since: query.since,
    until: query.until,
    by: "service",
  };
  const [cpuTime, memory, cgroupMemory] = await Promise.all([
    api.getMetricSeries(RESOURCE_METRIC_NAMES.cpuTime, metricQuery, signal),
    api.getMetricSeries(RESOURCE_METRIC_NAMES.memory, metricQuery, signal),
    api.getMetricSeries(RESOURCE_METRIC_NAMES.cgroupMemory, metricQuery, signal),
  ]);
  return { service: query.service, cpuTime, memory, cgroupMemory };
}

export function toResourceCharts(resources: ServiceResources): ResourceCharts {
  const frame = toMetricFrame(resources.cpuTime);
  const cpuSeries = toLabeledSeries(resources.cpuTime, "CPU", frame);
  const memorySeries = [
    ...toLabeledSeries(resources.memory, "Processes", frame),
    ...toLabeledSeries(resources.cgroupMemory, "Cgroup", frame),
  ];
  return cpuSeries.length === 0 && memorySeries.length === 0
    ? { state: "missing" }
    : { state: "shown", frame, cpuSeries, memorySeries };
}

function toLabeledSeries(metric: MetricSeries, label: string, frame: TimeFrame): TimeSeries[] {
  const [group] = metric.groups;
  return group ? [{ label, values: readAveragesOrRates(group, frame) }] : [];
}
