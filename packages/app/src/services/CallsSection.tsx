import { createMemo } from "solid-js";

import type { CallOperation, Calls, Target } from "@otelo/api";
import { ChartPanel, type Column, Panel, Table, type TimeSeries } from "@otelo/viz";

import KindBadge from "../traces/KindBadge";
import SpanTitle from "../traces/SpanTitle";
import { timeFrame } from "./frame";
import { PERCENTILES, requestSeries } from "./series";
import { rate, seconds, share } from "./stats";
import { summaryAttributes, targetLabel } from "./target";
import TargetName from "./TargetName";

/** How many targets the chart of time tells apart; the rest add up to one
 * series. */
const CHARTED = 7;

/** Calls that do the same thing, with the target they go to. */
export interface CallRow {
  target: Target;
  operation: CallOperation;
}

/**
 * The calls a service makes: the time they take by target and their count
 * over the range, and a table of what they do, such as one row for each
 * query with its values taken out, with the target of each.
 */
export default function CallsSection(props: {
  calls: Calls;
  loading: boolean;
  onZoom: (start: number, end: number) => void;
  callHref: (row: CallRow) => string;
  onOpenCall: (row: CallRow) => void;
}) {
  const rangeSeconds = () => seconds(props.calls.start_at, props.calls.end_at);

  const frame = createMemo(() => timeFrame(props.calls));
  const countChart = createMemo(() => requestSeries(props.calls.buckets));
  const timeChart = createMemo<TimeSeries[]>(() => {
    const targets = props.calls.targets;
    const series: TimeSeries[] = targets.slice(0, CHARTED).map((t) => ({
      label: targetLabel(t),
      values: t.buckets.map((b) => b.requests.total_ns),
    }));
    const rest = targets.slice(CHARTED);
    if (rest.length > 0) {
      series.push({
        label: `${rest.length} more`,
        color: "muted",
        values: props.calls.buckets.map((_, i) =>
          rest.reduce((sum, t) => sum + (t.buckets[i]?.requests.total_ns ?? 0), 0),
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
  const calls = (r: CallRow) => r.operation.calls;

  const columns: Column<CallRow>[] = [
    {
      id: "summary",
      label: "Call",
      width: "minmax(24ch,5fr)",
      value: (r) => r.operation.summary,
      cell: (r) => (
        <SpanTitle
          name={r.operation.summary}
          attributes={summaryAttributes(r.target.type, r.operation.summary, r.operation.attributes)}
          error={false}
          status={false}
        />
      ),
    },
    {
      id: "target",
      label: "Target",
      width: "minmax(10ch,max-content)",
      value: (r) => targetLabel(r.target),
      cell: (r) => <TargetName target={r.target} />,
    },
    {
      id: "kind",
      label: "Kind",
      width: "max-content",
      value: (r) => r.operation.kind,
      cell: (r) => <KindBadge kind={r.operation.kind} />,
    },
    {
      id: "calls",
      label: "Calls",
      unit: "count",
      meter: true,
      value: (r) => calls(r).count,
    },
    {
      id: "rate",
      label: "Rate",
      unit: "rate",
      value: (r) => rate(calls(r).count, rangeSeconds()),
    },
    {
      id: "errors",
      label: "Error rate",
      unit: "ratio",
      value: (r) => share(calls(r).errors, calls(r).count),
      tone: (r) => (calls(r).errors > 0 ? "error" : undefined),
    },
    ...PERCENTILES.map((p): Column<CallRow> => ({
      id: p,
      label: p.toUpperCase(),
      unit: "duration",
      value: (r) => calls(r).latency?.[p] ?? null,
    })),
    {
      id: "total",
      label: "Total time",
      description: "The durations of the calls added up",
      unit: "duration",
      meter: true,
      value: (r) => calls(r).total_ns,
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
          series={timeChart()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
        <ChartPanel
          title="Call count"
          description="Calls to databases, hosts, and other services, by step"
          kind="bar"
          unit="count"
          frame={frame()}
          series={countChart()}
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
