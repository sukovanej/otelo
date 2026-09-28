import { createSignal, For, Show } from "solid-js";
import type { Span } from "../api";
import { closedErrorRow, closedRow, header, openRow, row, waterfallColumns } from "../classes";
import { ChevronIcon } from "@siner/icons";
import Duration from "../Duration";
import { toggleRow } from "../row";
import Service from "../Service";
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
  onSelect: (span: Span | undefined) => void;
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

  return (
    <div class="font-mono text-sm">
      <div class={`${header} ${waterfallColumns}`} aria-hidden="true">
        <span>Span</span>
        <span>Service</span>
        <span class="text-right">Duration</span>
        <span class="@container relative h-[1lh]">
          <For each={TICKS}>
            {(tick) => (
              <span
                class={`absolute pl-1 normal-case ${tick.class}`}
                style={{ left: share(tick.share * length()) }}
              >
                <Duration nanos={Math.round(tick.share * length())} />
              </span>
            )}
          </For>
        </span>
      </div>
      <div role="tree">
        <For each={visibleRows(props.rows, collapsed())}>
          {(item) => {
            const span = item.span;
            const isSelected = () => props.selected === span.span_id;
            const isCollapsed = () => collapsed().has(span.span_id);
            return (
              <div
                class={`${row} ${waterfallColumns} border-b border-line py-1 ${
                  isSelected() ? openRow : span.error ? closedErrorRow : closedRow
                }`}
                aria-level={item.depth + 1}
                aria-selected={isSelected()}
                aria-expanded={item.children > 0 ? !isCollapsed() : undefined}
                {...toggleRow(() => props.onSelect(isSelected() ? undefined : span))}
                role="treeitem"
              >
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
                        fold(span.span_id);
                      }}
                    >
                      <ChevronIcon size={13} direction={isCollapsed() ? "right" : "down"} />
                    </button>
                  </Show>
                  <SpanTitle name={span.name} attributes={span.attributes} error={span.error} />
                  <Show when={isCollapsed()}>
                    <span class="shrink-0 text-muted">+{item.children}</span>
                  </Show>
                </span>
                <span class="text-muted">
                  <Service name={span.service} resource={span.resource} />
                </span>
                <span class="text-right">
                  <Duration nanos={span.duration_ns} align />
                </span>
                <span class="relative h-[1lh] self-center" style={grid}>
                  <span
                    class={`absolute inset-y-1 min-w-0.5 rounded-sm ${
                      span.error ? "bg-error" : "bg-accent"
                    }`}
                    style={{ left: share(item.offset), width: share(span.duration_ns) }}
                  />
                </span>
              </div>
            );
          }}
        </For>
      </div>
    </div>
  );
}
