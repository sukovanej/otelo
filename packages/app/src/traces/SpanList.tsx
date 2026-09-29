import type { TraceSpan } from "@otelo/api";
import { type Column, Table } from "@otelo/viz";

import Service from "../Service";
import { formatTime, parseTime } from "../time";
import KindBadge from "./KindBadge";
import { spanFailed } from "./span";
import SpanTitle from "./SpanTitle";

/** What tells a span apart, so it stays selected when a reload brings it
 * again. */
export const spanKey = (span: TraceSpan) => `${span.trace_id} ${span.span_id}`;

const COLUMNS: Column<TraceSpan>[] = [
  {
    id: "time",
    label: "Time",
    width: "max-content",
    value: (span) => span.started_at,
    tone: () => "muted",
    cell: (span) => (
      <time class="whitespace-nowrap" datetime={span.started_at}>
        {formatTime(parseTime(span.started_at))}
      </time>
    ),
  },
  {
    id: "service",
    label: "Service",
    width: "minmax(6ch,16ch)",
    value: (span) => span.service,
    tone: () => "muted",
    cell: (span) => <Service name={span.service} resource={span.resource} />,
  },
  {
    id: "name",
    label: "Name",
    width: "minmax(0,1fr)",
    value: (span) => span.name,
    cell: (span) => (
      <SpanTitle name={span.name} attributes={span.attributes} error={spanFailed(span)} />
    ),
  },
  {
    id: "kind",
    label: "Kind",
    width: "max-content",
    value: (span) => span.kind,
    cell: (span) => <KindBadge kind={span.kind} />,
  },
  {
    id: "duration",
    label: "Duration",
    unit: "duration",
    meter: true,
    width: "20ch",
    value: (span) => span.duration_ns,
  },
];

/** Spans, newest first. A click on a span selects it, and a click on the
 * selected one lets it go. */
export default function SpanList(props: {
  spans: TraceSpan[];
  /** The key of the selected span. */
  selected: string | undefined;
  onSelect: (span: TraceSpan | undefined) => void;
}) {
  return (
    <Table
      label="Spans"
      rows={props.spans}
      columns={COLUMNS}
      selected={(span) => props.selected === spanKey(span)}
      tone={(span) => (spanFailed(span) ? "error" : undefined)}
      onRowClick={(span) => props.onSelect(props.selected === spanKey(span) ? undefined : span)}
    />
  );
}
