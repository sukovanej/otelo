import type { TraceSummary } from "@otelo/api";
import { type Column, Table } from "@otelo/viz";

import ServiceName from "../ServiceName";
import { formatTime, parseTime } from "../time";
import SpanTitle from "./SpanTitle";

const COLUMNS: Column<TraceSummary>[] = [
  {
    id: "time",
    label: "Time",
    width: "max-content",
    value: (trace) => trace.started_at,
    tone: () => "muted",
    cell: (trace) => (
      <time class="whitespace-nowrap" datetime={trace.started_at}>
        {formatTime(parseTime(trace.started_at))}
      </time>
    ),
  },
  {
    id: "service",
    label: "Service",
    width: "minmax(6ch,16ch)",
    value: (trace) => trace.service,
    tone: () => "muted",
    cell: (trace) => <ServiceName name={trace.service} resource={trace.resource} />,
  },
  {
    id: "name",
    label: "Root span",
    width: "minmax(0,1fr)",
    value: (trace) => trace.name,
    cell: (trace) => (
      <SpanTitle
        variant="span"
        name={trace.name}
        attributes={trace.attributes}
        error={trace.error}
      />
    ),
  },
  { id: "spans", label: "Spans", unit: "count", value: (trace) => trace.spans },
  {
    id: "duration",
    label: "Duration",
    unit: "duration",
    meter: true,
    width: "20ch",
    value: (trace) => trace.duration_ns,
  },
];

interface TraceListProps {
  readonly traces: ReadonlyArray<TraceSummary>;
  readonly onOpen: (id: string) => void;
}

export default function TraceList(props: TraceListProps) {
  return (
    <Table
      label="Traces"
      rows={props.traces}
      columns={COLUMNS}
      tone={(trace) => (trace.error ? "error" : undefined)}
      href={(trace) => `/traces/${trace.trace_id}`}
      onRowClick={(trace) => props.onOpen(trace.trace_id)}
    />
  );
}
