import type { TraceSummary } from "@otelo/api";
import { type Column, Table } from "@otelo/viz";

import ServiceName from "../ServiceName";
import { formatTime, parseTime } from "../time";
import { type SpanSorting, toTableSorting } from "./sort";
import SpanTitle from "./SpanTitle";

const COLUMNS: Column<TraceSummary>[] = [
  {
    kind: "cell",
    id: "time",
    label: "Time",
    width: "max-content",
    tone: () => "muted",
    cell: (trace) => (
      <time class="whitespace-nowrap" datetime={trace.started_at}>
        {formatTime(parseTime(trace.started_at))}
      </time>
    ),
  },
  {
    kind: "cell",
    id: "service",
    label: "Service",
    width: "minmax(6ch,16ch)",
    tone: () => "muted",
    cell: (trace) => <ServiceName name={trace.service} resource={trace.resource} />,
  },
  {
    kind: "cell",
    id: "name",
    label: "Root span",
    width: "minmax(0,1fr)",
    cell: (trace) => (
      <SpanTitle
        variant="span"
        name={trace.name}
        attributes={trace.attributes}
        error={trace.error}
      />
    ),
  },
  { kind: "number", id: "spans", label: "Spans", unit: "count", value: (trace) => trace.spans },
  {
    kind: "meter",
    id: "duration",
    label: "Duration",
    unit: "duration",
    barWidth: "12ch",
    value: (trace) => trace.duration_ns,
  },
];

interface TraceListProps {
  readonly traces: ReadonlyArray<TraceSummary>;
  readonly onOpen: (id: string) => void;
  readonly sorting?: SpanSorting;
}

export default function TraceList(props: TraceListProps) {
  return (
    <Table
      label="Traces"
      rows={props.traces}
      rowKey={(trace) => trace.trace_id}
      columns={COLUMNS}
      sorting={props.sorting && toTableSorting(props.sorting)}
      tone={(trace) => (trace.error ? "error" : undefined)}
      href={(trace) => `/traces/${trace.trace_id}`}
      onRowClick={(trace) => props.onOpen(trace.trace_id)}
    />
  );
}
