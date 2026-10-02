import { createProjection, createSignal, For, Show } from "solid-js";

import type { TraceSpan } from "@otelo/api";
import { ChevronIcon } from "@otelo/icons";
import { type Column, Table, Value } from "@otelo/viz";

import ServiceName from "../../ServiceName";
import { isFailedSpan } from "../span";
import SpanTitle from "../SpanTitle";
import { dropCollapsedRows, measureTraceNanos, type TreeRow } from "../tree";

const AXIS_TICKS = [
  { share: 0, class: "" },
  { share: 0.25, class: "@max-[26rem]:hidden" },
  { share: 0.5, class: "@max-[13rem]:hidden" },
  { share: 0.75, class: "@max-[26rem]:hidden" },
];

const TICK_LINES_STYLE = {
  "background-image": "linear-gradient(to right, var(--color-line) 1px, transparent 1px)",
  "background-size": "25% 100%",
};

interface TraceViewWaterfallProps {
  readonly rows: ReadonlyArray<TreeRow>;
  readonly selectedSpanId: string | undefined;
  readonly onSelect: (span: TraceSpan | undefined) => void;
}

export default function TraceViewWaterfall(props: TraceViewWaterfallProps) {
  const [collapsedSpanIds, setCollapsedSpanIds] = createSignal<ReadonlySet<string>>(new Set());
  const collapsedSpans = createProjection<Record<string, true>>(
    () => Object.fromEntries([...collapsedSpanIds()].map((spanId) => [spanId, true])),
    {},
  );
  const traceNanos = () => Math.max(1, measureTraceNanos(props.rows));
  const toPercentOfTrace = (nanos: number) => `${(100 * nanos) / traceNanos()}%`;
  const toggleFold = (spanId: string) =>
    setCollapsedSpanIds((spanIds) => {
      const next = new Set(spanIds);
      if (!next.delete(spanId)) next.add(spanId);
      return next;
    });

  const columns: Column<TreeRow>[] = [
    {
      kind: "cell",
      id: "span",
      label: "Span",
      width: "minmax(28ch,2fr)",
      cell: (row) => {
        const isCollapsed = () => collapsedSpans[row.span.span_id] === true;
        return (
          <span
            class="flex min-w-0 items-baseline gap-1.5"
            style={{ "padding-left": `${row.depth * 1.5}ch` }}
          >
            <Show when={row.childCount > 0} fallback={<span class="w-[2ch] shrink-0" />}>
              <button
                type="button"
                class="flex w-[2ch] shrink-0 cursor-pointer justify-center self-center text-muted hover:text-ink"
                title={isCollapsed() ? `Show ${row.childCount} under it` : "Fold"}
                aria-label={isCollapsed() ? "Unfold" : "Fold"}
                onClick={(e) => {
                  e.stopPropagation();
                  toggleFold(row.span.span_id);
                }}
              >
                <ChevronIcon size={13} direction={isCollapsed() ? "right" : "down"} />
              </button>
            </Show>
            <SpanTitle
              variant="span"
              name={row.span.name}
              attributes={row.span.attributes}
              error={isFailedSpan(row.span)}
            />
            <Show when={isCollapsed()}>
              <span class="shrink-0 text-muted">+{row.childCount}</span>
            </Show>
          </span>
        );
      },
    },
    {
      kind: "cell",
      id: "service",
      label: "Service",
      width: "minmax(6ch,13ch)",
      tone: () => "muted",
      cell: (row) => <ServiceName name={row.span.service} resource={row.span.resource} />,
    },
    {
      kind: "number",
      id: "duration",
      label: "Duration",
      unit: "duration",
      value: (row) => row.span.duration_ns,
    },
    {
      kind: "cell",
      id: "timeline",
      label: "Timeline",
      width: "3fr",
      header: () => (
        <span class="@container relative h-[1lh] w-full">
          <For each={AXIS_TICKS}>
            {(tick) => (
              <span
                class={`absolute pl-1 font-mono normal-case ${tick.class}`}
                style={{ left: toPercentOfTrace(tick.share * traceNanos()) }}
              >
                <Value value={Math.round(tick.share * traceNanos())} unit="duration" />
              </span>
            )}
          </For>
        </span>
      ),
      cell: (row) => (
        <span class="relative h-[1lh] w-full self-center" style={TICK_LINES_STYLE}>
          <span
            class={`absolute inset-y-1 min-w-0.5 rounded-sm ${
              isFailedSpan(row.span) ? "bg-error" : "bg-accent"
            }`}
            style={{
              left: toPercentOfTrace(row.startOffsetNanos),
              width: toPercentOfTrace(row.span.duration_ns),
            }}
          />
        </span>
      ),
    },
  ];

  return (
    <Table
      label="Spans of the trace"
      rows={dropCollapsedRows(props.rows, collapsedSpanIds())}
      rowKey={(row) => row.span.span_id}
      columns={columns}
      level={(row) =>
        row.childCount > 0
          ? {
              kind: "branch",
              depth: row.depth,
              expanded: collapsedSpans[row.span.span_id] !== true,
            }
          : { kind: "leaf", depth: row.depth }
      }
      selectedKey={() => props.selectedSpanId}
      tone={(row) => (isFailedSpan(row.span) ? "error" : undefined)}
      onRowClick={(row) =>
        props.onSelect(props.selectedSpanId === row.span.span_id ? undefined : row.span)
      }
    />
  );
}
