import { createMemo, For } from "solid-js";

import type { Service } from "@otelo/api";
import { ChartPanel, formatValue, Stat, type TimeFrame, type TimeSeries } from "@otelo/viz";

import { toTimeFrame } from "../frame";
import { PERCENTILES, toCountSeries, toErrorRateSeries, toLatencySeries } from "../series";
import { measureSeconds, toRate, toShare } from "../stats";

interface ServicePageOverviewProps {
  readonly service: Service;
  readonly loading: boolean;
  readonly onZoom: (start: number, end: number) => void;
}

export default function ServicePageOverview(props: ServicePageOverviewProps) {
  const frame = createMemo<TimeFrame>(() => toTimeFrame(props.service));
  const buckets = () => props.service.buckets;
  const stats = () => props.service.stats;
  const requests = () => stats().requests;
  const rangeSeconds = () => measureSeconds(props.service.start_at, props.service.end_at);

  // A prop written as an array literal would build the series again on every read.
  const requestSteps = createMemo(() => buckets().map((bucket) => bucket.requests));
  const requestSeries = createMemo(() => toCountSeries(requestSteps()));
  const latencySeries = createMemo(() => toLatencySeries(requestSteps()));
  const errorRateSeries = createMemo(() => toErrorRateSeries(requestSteps()));
  const requestCountTrend = createMemo(() => buckets().map((bucket) => bucket.requests.count));
  const logCountTrend = createMemo(() => buckets().map((bucket) => bucket.logs));
  const logSeries = createMemo<TimeSeries[]>(() => [
    {
      label: "Below error",
      color: "muted",
      values: buckets().map((bucket) => bucket.logs - bucket.error_logs),
    },
    {
      label: "Error and above",
      color: "error",
      values: buckets().map((bucket) => bucket.error_logs),
    },
  ]);

  return (
    <>
      <div class="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
        <Stat
          label="Requests"
          value={requests().count}
          unit="count"
          detail={formatValue(toRate(requests().count, rangeSeconds()), "rate")}
          trend={requestCountTrend()}
        />
        <Stat
          label="Error rate"
          value={toShare(requests().errors, requests().count)}
          unit="ratio"
          tone={requests().errors > 0 ? "error" : undefined}
          detail={`${requests().errors.toLocaleString()} failed`}
          trend={errorRateSeries()[0]?.values}
          trendColor="error"
        />
        <For each={PERCENTILES}>
          {(percentile, index) => (
            <Stat
              label={`${percentile.toUpperCase()} latency`}
              value={requests().latency?.[percentile]}
              unit="duration"
              trend={latencySeries()[index()]?.values}
              trendColor={percentile}
            />
          )}
        </For>
        <Stat
          label="Logs"
          value={stats().logs}
          unit="count"
          detail={`${stats().error_logs.toLocaleString()} errors`}
          trend={logCountTrend()}
          trendColor="muted"
        />
      </div>

      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <ChartPanel
          title="Requests"
          description="HTTP requests the service served, by step"
          kind="bar"
          unit="count"
          frame={frame()}
          series={requestSeries()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
        <ChartPanel
          title="Latency"
          description="Percentiles of the durations of the requests"
          kind="line"
          unit="duration"
          frame={frame()}
          series={latencySeries()}
          loading={props.loading}
          onZoom={props.onZoom}
          emptyMessage="No requests in this range"
        />
        <ChartPanel
          title="Error rate"
          description="The share of the requests that failed"
          kind="area"
          unit="ratio"
          frame={frame()}
          series={errorRateSeries()}
          loading={props.loading}
          onZoom={props.onZoom}
          emptyMessage="No requests in this range"
        />
        <ChartPanel
          title="Logs"
          description="Log lines by level, by step"
          kind="bar"
          unit="count"
          frame={frame()}
          series={logSeries()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
      </div>
    </>
  );
}
