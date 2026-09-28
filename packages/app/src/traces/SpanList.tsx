import { For } from "solid-js";

import type { TraceSpan } from "@siner/api";

import { closedErrorRow, closedRow, header, openRow, row, spanColumns } from "../classes";
import Duration from "../Duration";
import Measure from "../Measure";
import { toggleRow } from "../row";
import Service from "../Service";
import { formatTime, parseTime, timeWidth } from "../time";
import KindBadge from "./KindBadge";
import SpanTitle from "./SpanTitle";

/** What tells a span apart, so it stays selected when a reload brings it
 * again. */
export const spanKey = (span: TraceSpan) => `${span.trace_id} ${span.span_id}`;

/** Spans, newest first. A click on a span selects it, and a click on the
 * selected one lets it go. */
export default function SpanList(props: {
  spans: TraceSpan[];
  /** The key of the selected span. */
  selected: string | undefined;
  onSelect: (span: TraceSpan | undefined) => void;
}) {
  const longest = () => Math.max(1, ...props.spans.map((span) => span.duration_ns));
  return (
    <div
      class="font-mono text-sm"
      style={{ "--time-width": timeWidth(props.spans.map((span) => span.time)) }}
    >
      <div class={`${header} ${spanColumns}`} aria-hidden="true">
        <span>Time</span>
        <span>Service</span>
        <span>Name</span>
        <span>Kind</span>
        <span class="text-right">Duration</span>
      </div>
      <div role="list">
        <For each={props.spans}>
          {(span) => {
            const isSelected = () => props.selected === spanKey(span);
            return (
              <div class="border-b border-line" role="listitem">
                <div
                  class={`${row} ${spanColumns} py-1.5 ${
                    isSelected() ? openRow : span.error ? closedErrorRow : closedRow
                  }`}
                  aria-pressed={isSelected()}
                  {...toggleRow(() => props.onSelect(isSelected() ? undefined : span))}
                >
                  <time class="whitespace-nowrap text-muted" datetime={span.time}>
                    {formatTime(parseTime(span.time))}
                  </time>
                  <span class="text-muted">
                    <Service name={span.service} resource={span.resource} />
                  </span>
                  <SpanTitle name={span.name} attributes={span.attributes} error={span.error} />
                  <KindBadge kind={span.kind} />
                  <Measure share={span.duration_ns / longest()}>
                    <Duration nanos={span.duration_ns} align />
                  </Measure>
                </div>
              </div>
            );
          }}
        </For>
      </div>
    </div>
  );
}
