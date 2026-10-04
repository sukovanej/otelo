import type { TraceSpan } from "@otelo/api";
import { SpanKindBadge } from "@otelo/ui";
import { type Column, Table } from "@otelo/viz";

import ServiceName from "../ServiceName";
import { formatTime, parseTime } from "../time";
import { type SpanSorting, toTableSorting } from "./sort";
import { isFailedSpan, toKindName } from "./span";
import SpanTitle from "./SpanTitle";

const COLUMNS: Column<TraceSpan>[] = [
  {
    kind: "cell",
    id: "time",
    label: "Time",
    width: "max-content",
    tone: () => "muted",
    cell: (span) => (
      <time class="whitespace-nowrap" datetime={span.started_at}>
        {formatTime(parseTime(span.started_at))}
      </time>
    ),
  },
  {
    kind: "cell",
    id: "service",
    label: "Service",
    width: "minmax(6ch,16ch)",
    tone: () => "muted",
    cell: (span) => <ServiceName name={span.service} resource={span.resource} />,
  },
  {
    kind: "cell",
    id: "name",
    label: "Name",
    width: "minmax(0,1fr)",
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
    kind: "cell",
    id: "kind",
    label: "Kind",
    width: "max-content",
    cell: (span) => <SpanKindBadge kind={toKindName(span.kind)} />,
  },
  {
    kind: "meter",
    id: "duration",
    label: "Duration",
    unit: "duration",
    barWidth: "12ch",
    value: (span) => span.duration_ns,
  },
];

interface SpanListProps {
  readonly spans: ReadonlyArray<TraceSpan>;
  readonly selectedKey: string | undefined;
  readonly onSelect: (span: TraceSpan | undefined) => void;
  readonly sorting?: SpanSorting;
}

export default function SpanList(props: SpanListProps) {
  return (
    <Table
      label="Spans"
      rows={props.spans}
      rowKey={toSpanKey}
      columns={COLUMNS}
      sorting={props.sorting && toTableSorting(props.sorting)}
      selectedKey={() => props.selectedKey}
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
