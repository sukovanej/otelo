import type { TraceSpan } from "@otelo/api";
import { type Column, Table } from "@otelo/viz";

import ServiceName from "../ServiceName";
import { formatTime, parseTime } from "../time";
import { isFailedSpan, toKindName } from "./span";
import SpanKindBadge from "./SpanKindBadge";
import SpanTitle from "./SpanTitle";

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
    cell: (span) => <ServiceName name={span.service} resource={span.resource} />,
  },
  {
    id: "name",
    label: "Name",
    width: "minmax(0,1fr)",
    value: (span) => span.name,
    cell: (span) => (
      <SpanTitle
        variant="span"
        name={span.name}
        attributes={span.attributes}
        error={isFailedSpan(span)}
      />
    ),
  },
  {
    id: "kind",
    label: "Kind",
    width: "max-content",
    value: (span) => span.kind,
    cell: (span) => <SpanKindBadge kind={toKindName(span.kind)} />,
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

interface SpanListProps {
  readonly spans: ReadonlyArray<TraceSpan>;
  readonly selectedKey: string | undefined;
  readonly onSelect: (span: TraceSpan | undefined) => void;
}

export default function SpanList(props: SpanListProps) {
  return (
    <Table
      label="Spans"
      rows={props.spans}
      columns={COLUMNS}
      selected={(span) => props.selectedKey === toSpanKey(span)}
      tone={(span) => (isFailedSpan(span) ? "error" : undefined)}
      onRowClick={(span) =>
        props.onSelect(props.selectedKey === toSpanKey(span) ? undefined : span)
      }
    />
  );
}

export function toSpanKey(span: TraceSpan): string {
  return `${span.trace_id} ${span.span_id}`;
}
