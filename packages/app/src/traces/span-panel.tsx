import { For, Show } from "solid-js";

import type { TraceSpan } from "@otelo/api";
import { isPlainLeftClick, SidePanel, SpanKindBadge } from "@otelo/ui";
import { Value } from "@otelo/viz";

import { link, sectionHeading, timesLine } from "../classes";
import { toUnnamedField } from "../field";
import FieldSections from "../FieldSections";
import FieldTable from "../FieldTable";
import ServiceName from "../ServiceName";
import { formatDateTime, formatTime, measureNanosSince, parseTime } from "../time";
import { isFailedSpan, listSpanSections, toKindName } from "./span";
import SpanTitle from "./SpanTitle";

interface SpanPanelProps {
  readonly span: TraceSpan;
  readonly onOpenTrace: ((span: TraceSpan) => void) | undefined;
  readonly onFilter: (term: string) => void;
  readonly onClose: () => void;
}

export default function SpanPanel(props: SpanPanelProps) {
  return (
    <SidePanel
      label="Span"
      onClose={props.onClose}
      header={
        <>
          <SpanKindBadge kind={toKindName(props.span.kind)} />
          <span class="font-mono text-sm">{formatTime(parseTime(props.span.started_at))}</span>
          <span class="min-w-0 font-mono text-sm text-muted">
            <ServiceName name={props.span.service} resource={props.span.resource} />
          </span>
          <Show when={props.onOpenTrace !== undefined}>
            <a
              href={`/traces/${props.span.trace_id}?span=${props.span.span_id}`}
              class={link}
              onClick={(e) => {
                if (!props.onOpenTrace || !isPlainLeftClick(e)) return;
                e.preventDefault();
                props.onOpenTrace(props.span);
              }}
            >
              Trace
            </a>
          </Show>
        </>
      }
    >
      <div class="font-mono">
        <div class="mb-1 text-md font-semibold">
          <SpanTitle
            variant="span"
            name={props.span.name}
            attributes={props.span.attributes}
            error={isFailedSpan(props.span)}
          />
        </div>
        <div class={timesLine}>
          <Value value={props.span.duration_ns} unit="duration" /> from{" "}
          {formatDateTime(parseTime(props.span.started_at))} local · {props.span.started_at}
        </div>
        <FieldSections
          sections={listSpanSections(props.span)}
          pluralNoun="spans"
          onFilter={props.onFilter}
        />
        <Show when={props.span.events.length > 0}>
          <section>
            <h3 class={sectionHeading}>Events</h3>
            <For each={props.span.events}>
              {(event) => (
                <div class="mb-2">
                  <div>
                    <span class="text-muted">
                      +
                      <Value
                        value={measureNanosSince(props.span.started_at, event.occurred_at)}
                        unit="duration"
                      />
                    </span>{" "}
                    {event.name}
                  </div>
                  <div class="pl-2">
                    <FieldTable
                      fields={Object.entries(event.attributes).map(([key, value]) =>
                        toUnnamedField(key, value),
                      )}
                      pluralNoun="spans"
                      onFilter={props.onFilter}
                    />
                  </div>
                </div>
              )}
            </For>
          </section>
        </Show>
      </div>
    </SidePanel>
  );
}
