import { createMemo } from "solid-js";

import type { CallOperation, Calls, Target } from "@otelo/api";
import { ChartPanel, type Column, Panel, Table, type TimeSeries } from "@otelo/viz";

import { toKindName } from "../../traces/span";
import SpanKindBadge from "../../traces/SpanKindBadge";
import SpanTitle from "../../traces/SpanTitle";
import { toTimeFrame } from "../frame";
import { PERCENTILES, toRequestSeries } from "../series";
import { measureSeconds, toRate, toShare } from "../stats";
import { applySummaryToAttributes, toTargetLabel } from "../target";
import ServicePageTargetName from "./service-page-target-name";

const MAX_CHARTED_TARGETS = 7;

export interface CallRow {
  readonly target: Target;
  readonly operation: CallOperation;
}

interface ServicePageCallsProps {
  readonly calls: Calls;
  readonly loading: boolean;
  readonly onZoom: (start: number, end: number) => void;
  readonly callHref: (row: CallRow) => string;
  readonly onOpenCall: (row: CallRow) => void;
}

export default function ServicePageCalls(props: ServicePageCallsProps) {
  const rangeSeconds = () => measureSeconds(props.calls.start_at, props.calls.end_at);

  const frame = createMemo(() => toTimeFrame(props.calls));
  const countSeries = createMemo(() => toRequestSeries(props.calls.buckets));
  const timeSeries = createMemo<TimeSeries[]>(() => {
    const targets = props.calls.targets;
    const series: TimeSeries[] = targets.slice(0, MAX_CHARTED_TARGETS).map((target) => ({
      label: toTargetLabel(target),
      values: target.buckets.map((bucket) => bucket.requests.total_ns),
    }));
    const unchartedTargets = targets.slice(MAX_CHARTED_TARGETS);
    if (unchartedTargets.length > 0) {
      series.push({
        label: `${unchartedTargets.length} more`,
        color: "muted",
        values: props.calls.buckets.map((_, index) =>
          unchartedTargets.reduce(
            (sum, target) => sum + (target.buckets[index]?.requests.total_ns ?? 0),
            0,
          ),
        ),
      });
    }
    return series;
  });

  const rows = createMemo<CallRow[]>(() =>
    props.calls.targets.flatMap((target) =>
      target.operations.map((operation) => ({ target, operation })),
    ),
  );

  const columns: Column<CallRow>[] = [
    {
      id: "summary",
      label: "Call",
      width: "minmax(24ch,5fr)",
      value: (row) => row.operation.summary,
      cell: (row) => (
        <SpanTitle
          variant="operation"
          name={row.operation.summary}
          attributes={applySummaryToAttributes(
            row.target.type,
            row.operation.summary,
            row.operation.attributes,
          )}
        />
      ),
    },
    {
      id: "target",
      label: "Target",
      width: "minmax(10ch,max-content)",
      value: (row) => toTargetLabel(row.target),
      cell: (row) => <ServicePageTargetName target={row.target} />,
    },
    {
      id: "kind",
      label: "Kind",
      width: "max-content",
      value: (row) => row.operation.kind,
      cell: (row) => <SpanKindBadge kind={toKindName(row.operation.kind)} />,
    },
    {
      id: "calls",
      label: "Calls",
      unit: "count",
      meter: true,
      value: (row) => row.operation.calls.count,
    },
    {
      id: "rate",
      label: "Rate",
      unit: "rate",
      value: (row) => toRate(row.operation.calls.count, rangeSeconds()),
    },
    {
      id: "errors",
      label: "Error rate",
      unit: "ratio",
      value: (row) => toShare(row.operation.calls.errors, row.operation.calls.count),
      tone: (row) => (row.operation.calls.errors > 0 ? "error" : undefined),
    },
    ...PERCENTILES.map((percentile): Column<CallRow> => ({
      id: percentile,
      label: percentile.toUpperCase(),
      unit: "duration",
      value: (row) => row.operation.calls.latency?.[percentile] ?? null,
    })),
    {
      id: "total",
      label: "Total time",
      description: "The durations of the calls added up",
      unit: "duration",
      meter: true,
      value: (row) => row.operation.calls.total_ns,
    },
  ];

  return (
    <>
      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <ChartPanel
          title="Time in calls"
          description="The durations of the calls added up, by target"
          kind="bar"
          unit="duration"
          frame={frame()}
          series={timeSeries()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
        <ChartPanel
          title="Call count"
          description="Calls to databases, hosts, and other services, by step"
          kind="bar"
          unit="count"
          frame={frame()}
          series={countSeries()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
      </div>

      <Panel
        title="Calls"
        description={
          props.calls.truncated
            ? "What the calls do, with their values taken out; the ones with the most time only"
            : "What the calls do, with their values taken out"
        }
        flush
      >
        <Table
          label="Calls"
          rows={rows()}
          columns={columns}
          sort={{ column: "total", descending: true }}
          href={props.callHref}
          onRowClick={props.onOpenCall}
          loading={props.loading}
        />
      </Panel>
    </>
  );
}
