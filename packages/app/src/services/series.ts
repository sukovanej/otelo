import type { SpanStats } from "@otelo/api";
import type { TimeSeries } from "@otelo/viz";

import { toShare } from "./stats";

export const PERCENTILES = ["p50", "p95", "p99"] as const;

export function toCountSeries(steps: ReadonlyArray<SpanStats>): TimeSeries[] {
  return [
    {
      label: "OK",
      color: "series-1",
      values: steps.map((step) => step.count - step.errors),
    },
    { label: "Failed", color: "error", values: steps.map((step) => step.errors) },
  ];
}

export function toLatencySeries(steps: ReadonlyArray<SpanStats>): TimeSeries[] {
  return PERCENTILES.map((percentile) => ({
    label: percentile.toUpperCase(),
    color: percentile,
    values: steps.map((step) => step.latency?.[percentile] ?? null),
  }));
}

export function toErrorRateSeries(steps: ReadonlyArray<SpanStats>): TimeSeries[] {
  return [
    {
      label: "Error rate",
      color: "error",
      values: steps.map((step) => toShare(step.errors, step.count)),
    },
  ];
}
