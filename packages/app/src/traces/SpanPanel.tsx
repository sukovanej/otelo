import { A } from "@solidjs/router";
import { For, Show } from "solid-js";
import type { Span } from "../api";
import { heading, link, times } from "../classes";
import Duration from "../Duration";
import FieldTable, { Fields } from "../FieldTable";
import Panel from "../Panel";
import Service from "../Service";
import { plainClick } from "../row";
import { formatDateTime, formatTime, nanosAfter, parseTime } from "../time";
import KindBadge from "./KindBadge";
import { spanSections } from "./span";
import SpanTitle from "./SpanTitle";

/** One span in full, in a panel beside a list, with a link to its trace
 * unless `inTrace` says the page shows it. A plain click on the link calls
 * `onOpenTrace` when there is one. */
export default function SpanPanel(props: {
  span: Span;
  inTrace?: boolean;
  onOpenTrace?: (span: Span) => void;
  onFilter: (term: string) => void;
  onClose: () => void;
}) {
  return (
    <Panel
      label="Span"
      onClose={props.onClose}
      header={
        <>
          <KindBadge kind={props.span.kind} />
          <span class="font-mono text-sm">{formatTime(parseTime(props.span.time))}</span>
          <span class="min-w-0 font-mono text-sm text-muted">
            <Service name={props.span.service} resource={props.span.resource} />
          </span>
          <Show when={!props.inTrace}>
            <A
              href={`/traces/${props.span.trace_id}?span=${props.span.span_id}`}
              class={link}
              onClick={(e) => {
                if (!props.onOpenTrace || !plainClick(e)) return;
                e.preventDefault();
                props.onOpenTrace(props.span);
              }}
            >
              Trace
            </A>
          </Show>
        </>
      }
    >
      <div class="font-mono">
        <div class="mb-1 text-md font-semibold">
          <SpanTitle
            name={props.span.name}
            attributes={props.span.attributes}
            error={props.span.error}
          />
        </div>
        <div class={times}>
          <Duration nanos={props.span.duration_ns} /> from{" "}
          {formatDateTime(parseTime(props.span.time))} local · {props.span.time}
        </div>
        <FieldTable sections={spanSections(props.span)} noun="spans" onFilter={props.onFilter} />
        <Show when={props.span.events.length > 0}>
          <section>
            <h3 class={heading}>Events</h3>
            <For each={props.span.events}>
              {(event) => (
                <div class="mb-2">
                  <div>
                    <span class="text-muted">
                      +<Duration nanos={nanosAfter(props.span.time, event.ts)} />
                    </span>{" "}
                    {event.name}
                  </div>
                  <div class="pl-2">
                    <Fields
                      fields={Object.entries(event.attributes).map(([key, value]) => ({
                        name: undefined,
                        label: key,
                        value,
                        literal: undefined,
                      }))}
                      noun="spans"
                      onFilter={props.onFilter}
                    />
                  </div>
                </div>
              )}
            </For>
          </section>
        </Show>
      </div>
    </Panel>
  );
}
