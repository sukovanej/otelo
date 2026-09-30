import type { RequestBucket } from "@otelo/api";
import type { TimeSeries } from "@otelo/viz";

import { toShare } from "./stats";

export const PERCENTILES = ["p50", "p95", "p99"] as const;

export function toRequestSeries(buckets: ReadonlyArray<RequestBucket>): TimeSeries[] {
  return [
    {
      label: "OK",
      color: "series-1",
      values: buckets.map((bucket) => bucket.requests.count - bucket.requests.errors),
    },
    { label: "Failed", color: "error", values: buckets.map((bucket) => bucket.requests.errors) },
  ];
}

export function toLatencySeries(buckets: ReadonlyArray<RequestBucket>): TimeSeries[] {
  return PERCENTILES.map((percentile) => ({
    label: percentile.toUpperCase(),
    color: percentile,
    values: buckets.map((bucket) => bucket.requests.latency?.[percentile] ?? null),
  }));
}

export function toErrorRateSeries(buckets: ReadonlyArray<RequestBucket>): TimeSeries[] {
  return [
    {
      label: "Error rate",
      color: "error",
      values: buckets.map((bucket) => toShare(bucket.requests.errors, bucket.requests.count)),
    },
  ];
}
