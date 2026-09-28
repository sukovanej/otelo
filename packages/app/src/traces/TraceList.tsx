import { A } from "@solidjs/router";
import { For } from "solid-js";
import type { TraceSummary } from "../api";
import { closedErrorRow, closedRow, header, row, traceColumns } from "../classes";
import Duration from "../Duration";
import Measure from "../Measure";
import { plainClick } from "../row";
import Service from "../Service";
import { formatTime, parseTime, timeWidth } from "../time";
import SpanTitle from "./SpanTitle";

/** Traces by their root span, newest first. A click opens a trace with
 * `onOpen`; each row still links to the page of its trace, for a new tab. */
export default function TraceList(props: { traces: TraceSummary[]; onOpen: (id: string) => void }) {
  const longest = () => Math.max(1, ...props.traces.map((trace) => trace.duration_ns));
  return (
    <div
      class="font-mono text-sm"
      style={{ "--time-width": timeWidth(props.traces.map((trace) => trace.time)) }}
    >
      <div class={`${header} ${traceColumns}`} aria-hidden="true">
        <span>Time</span>
        <span>Service</span>
        <span>Root span</span>
        <span class="text-right">Spans</span>
        <span class="text-right">Duration</span>
      </div>
      <div role="list">
        <For each={props.traces}>
          {(trace) => (
            <div class="border-b border-line" role="listitem">
              <A
                href={`/traces/${trace.trace_id}`}
                onClick={(e) => {
                  if (!plainClick(e)) return;
                  e.preventDefault();
                  props.onOpen(trace.trace_id);
                }}
                class={`${row} ${traceColumns} py-1.5 ${trace.error ? closedErrorRow : closedRow}`}
              >
                <time class="whitespace-nowrap text-muted" datetime={trace.time}>
                  {formatTime(parseTime(trace.time))}
                </time>
                <span class="text-muted">
                  <Service name={trace.service} resource={trace.resource} />
                </span>
                <SpanTitle name={trace.name} attributes={trace.attributes} error={trace.error} />
                <span class="text-right text-muted">{trace.spans.toLocaleString()}</span>
                <Measure share={trace.duration_ns / longest()}>
                  <Duration nanos={trace.duration_ns} align />
                </Measure>
              </A>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
