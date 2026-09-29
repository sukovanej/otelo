import { createSignal, For, Show } from "solid-js";

import type { TraceSpan } from "@siner/api";
import { ChevronIcon } from "@siner/icons";
import { type Column, Table, Value } from "@siner/viz";

import Service from "../Service";
import { spanFailed } from "./span";
import SpanTitle from "./SpanTitle";
import { traceLength, type TreeRow, visibleRows } from "./tree";

/** The ticks of the time axis, as shares of the trace, with the classes that
 * hide the ones between when the axis is too narrow for their labels. */
const TICKS = [
  { share: 0, class: "" },
  { share: 0.25, class: "@max-[26rem]:hidden" },
  { share: 0.5, class: "@max-[13rem]:hidden" },
  { share: 0.75, class: "@max-[26rem]:hidden" },
];

/** Lines under the ticks, behind the bars. */
const grid = {
  "background-image": "linear-gradient(to right, var(--color-line) 1px, transparent 1px)",
  "background-size": "25% 100%",
};

/**
 * The spans of a trace as a tree, each with a bar where it runs on the time
 * of the trace. A span with children folds them away. A click on a span
 * selects it, and a click on the selected one lets it go.
 */
export default function Waterfall(props: {
  rows: TreeRow[];
  /** The ID of the selected span. */
  selected: string | undefined;
  onSelect: (span: TraceSpan | undefined) => void;
}) {
  const [collapsed, setCollapsed] = createSignal<ReadonlySet<string>>(new Set());
  const length = () => Math.max(1, traceLength(props.rows));
  const share = (nanos: number) => `${(100 * nanos) / length()}%`;
  const fold = (id: string) =>
    setCollapsed((ids) => {
      const next = new Set(ids);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const columns: Column<TreeRow>[] = [
    {
      id: "span",
      label: "Span",
      width: "minmax(28ch,2fr)",
      value: (item) => item.span.name,
      cell: (item) => {
        const isCollapsed = () => collapsed().has(item.span.span_id);
        return (
          <span
            class="flex min-w-0 items-baseline gap-1.5"
            style={{ "padding-left": `${item.depth * 1.5}ch` }}
          >
            <Show when={item.children > 0} fallback={<span class="w-[2ch] shrink-0" />}>
              <button
                type="button"
                class="flex w-[2ch] shrink-0 cursor-pointer justify-center self-center text-muted hover:text-ink"
                title={isCollapsed() ? `Show ${item.children} under it` : "Fold"}
                aria-label={isCollapsed() ? "Unfold" : "Fold"}
                onClick={(e) => {
                  e.stopPropagation();
                  fold(item.span.span_id);
                }}
              >
                <ChevronIcon size={13} direction={isCollapsed() ? "right" : "down"} />
              </button>
            </Show>
            <SpanTitle
              name={item.span.name}
              attributes={item.span.attributes}
              error={spanFailed(item.span)}
            />
            <Show when={isCollapsed()}>
              <span class="shrink-0 text-muted">+{item.children}</span>
            </Show>
          </span>
        );
      },
    },
    {
      id: "service",
      label: "Service",
      width: "minmax(6ch,13ch)",
      value: (item) => item.span.service,
      tone: () => "muted",
      cell: (item) => <Service name={item.span.service} resource={item.span.resource} />,
    },
    {
      id: "duration",
      label: "Duration",
      unit: "duration",
      value: (item) => item.span.duration_ns,
    },
    {
      id: "timeline",
      label: "Timeline",
      width: "3fr",
      value: (item) => item.offset,
      header: () => (
        <span class="@container relative h-[1lh] w-full">
          <For each={TICKS}>
            {(tick) => (
              <span
                class={`absolute pl-1 font-mono normal-case ${tick.class}`}
                style={{ left: share(tick.share * length()) }}
              >
                <Value value={Math.round(tick.share * length())} unit="duration" />
              </span>
            )}
          </For>
        </span>
      ),
      cell: (item) => (
        <span class="relative h-[1lh] w-full self-center" style={grid}>
          <span
            class={`absolute inset-y-1 min-w-0.5 rounded-sm ${
              spanFailed(item.span) ? "bg-error" : "bg-accent"
            }`}
            style={{ left: share(item.offset), width: share(item.span.duration_ns) }}
          />
        </span>
      ),
    },
  ];

  return (
    <Table
      label="Spans of the trace"
      rows={visibleRows(props.rows, collapsed())}
      columns={columns}
      level={(item) => ({
        depth: item.depth,
        expanded: item.children > 0 ? !collapsed().has(item.span.span_id) : undefined,
      })}
      selected={(item) => props.selected === item.span.span_id}
      tone={(item) => (spanFailed(item.span) ? "error" : undefined)}
      onRowClick={(item) =>
        props.onSelect(props.selected === item.span.span_id ? undefined : item.span)
      }
    />
  );
}
