// The series of the charts of requests, from the buckets of the services
// API: of a service and of one of its operations alike.

import type { Requests } from "@otelo/api";
import type { TimeSeries } from "@otelo/viz";

import { share } from "./stats";

export const PERCENTILES = ["p50", "p95", "p99"] as const;

type Buckets = { requests: Requests }[];

/** The requests of each step, the failed ones on top. */
export const requestSeries = (buckets: Buckets): TimeSeries[] => [
  {
    label: "OK",
    color: "series-1",
    values: buckets.map((b) => b.requests.count - b.requests.errors),
  },
  { label: "Failed", color: "error", values: buckets.map((b) => b.requests.errors) },
];

/** The percentiles of the durations of the requests of each step. */
export const latencySeries = (buckets: Buckets): TimeSeries[] =>
  PERCENTILES.map((p) => ({
    label: p.toUpperCase(),
    color: p,
    values: buckets.map((b) => b.requests.latency?.[p] ?? null),
  }));

/** The share of the requests of each step that failed. */
export const errorRateSeries = (buckets: Buckets): TimeSeries[] => [
  {
    label: "Error rate",
    color: "error",
    values: buckets.map((b) => share(b.requests.errors, b.requests.count)),
  },
];
