import { createMemo, For, Show } from "solid-js";

import type { Operation, Service } from "@otelo/api";
import { SpanKindBadge } from "@otelo/ui";
import {
  ChartPanel,
  type Column,
  formatValue,
  Panel,
  Stat,
  Table,
  type TimeFrame,
  type TimeSeries,
} from "@otelo/viz";

import { toKindName } from "../../traces/span";
import SpanTitle from "../../traces/SpanTitle";
import { toTimeFrame } from "../frame";
import { PERCENTILES, toErrorRateSeries, toLatencySeries, toRequestSeries } from "../series";
import { measureSeconds, toRate, toShare } from "../stats";

interface ServicePageOverviewProps {
  readonly service: Service;
  readonly loading: boolean;
  readonly onZoom: (start: number, end: number) => void;
  readonly operationHref: (operation: Operation) => string;
  readonly onOpenOperation: (operation: Operation) => void;
}

export default function ServicePageOverview(props: ServicePageOverviewProps) {
  const frame = createMemo<TimeFrame>(() => toTimeFrame(props.service));
  const buckets = () => props.service.buckets;
  const stats = () => props.service.stats;
  const requests = () => stats().requests;
  const rangeSeconds = () => measureSeconds(props.service.start_at, props.service.end_at);

  // A prop written as an array literal would build the series again on every read.
  const requestSeries = createMemo(() => toRequestSeries(buckets()));
  const latencySeries = createMemo(() => toLatencySeries(buckets()));
  const errorRateSeries = createMemo(() => toErrorRateSeries(buckets()));
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

  const operationColumns = createMemo<Column<Operation>[]>(() => [
    {
      kind: "cell",
      id: "name",
      label: "Operation",
      sortBy: (operation) => operation.name,
      cell: (operation) => (
        <SpanTitle variant="operation" name={operation.name} attributes={operation.attributes} />
      ),
    },
    {
      kind: "cell",
      id: "kind",
      label: "Kind",
      width: "max-content",
      sortBy: (operation) => operation.kind,
      cell: (operation) => <SpanKindBadge kind={toKindName(operation.kind)} />,
    },
    {
      kind: "meter",
      id: "requests",
      label: "Requests",
      unit: "count",
      value: (operation) => operation.requests.count,
    },
    {
      kind: "number",
      id: "rate",
      label: "Rate",
      unit: "rate",
      value: (operation) => toRate(operation.requests.count, rangeSeconds()),
    },
    {
      kind: "number",
      id: "errors",
      label: "Error rate",
      unit: "ratio",
      value: (operation) => toShare(operation.requests.errors, operation.requests.count),
      tone: (operation) => (operation.requests.errors > 0 ? "error" : undefined),
    },
    ...PERCENTILES.map((percentile): Column<Operation> => ({
      kind: "number",
      id: percentile,
      label: percentile.toUpperCase(),
      unit: "duration",
      value: (operation) => operation.requests.latency?.[percentile] ?? null,
    })),
    {
      kind: "meter",
      id: "total",
      label: "Total time",
      description: "The durations of its requests added up",
      unit: "duration",
      value: (operation) => operation.requests.total_ns,
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
          trend={buckets().map((bucket) => bucket.requests.count)}
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
          trend={buckets().map((bucket) => bucket.logs)}
          trendColor="muted"
        />
      </div>

      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <ChartPanel
          title="Requests"
          description="Spans that enter the service, by step"
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

      <Show when={props.service.operations.length > 0}>
        <Panel
          title="Operations"
          description={
            props.service.truncated
              ? "The requests by span name, the most requested only"
              : "The requests by span name"
          }
          flush
        >
          <Table
            label="Operations"
            rows={props.service.operations}
            columns={operationColumns()}
            initialSort={{ columnId: "requests", descending: true }}
            href={props.operationHref}
            onRowClick={props.onOpenOperation}
            loading={props.loading}
          />
        </Panel>
      </Show>
    </>
  );
}
