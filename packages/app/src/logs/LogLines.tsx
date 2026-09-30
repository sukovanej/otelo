import type { LogLine } from "@otelo/api";
import { Level } from "@otelo/ui";
import { type Column, Table } from "@otelo/viz";

import ServiceName from "../ServiceName";
import { formatTime, parseTime } from "../time";
import { toLevelName } from "./level";

const MIN_ERROR_SEVERITY = 17;

const COLUMNS: Column<LogLine>[] = [
  {
    id: "time",
    label: "Time",
    width: "max-content",
    value: (line) => line.logged_at,
    tone: () => "muted",
    cell: (line) => (
      <time class="whitespace-nowrap" datetime={line.logged_at}>
        {formatTime(parseTime(line.logged_at))}
      </time>
    ),
  },
  {
    id: "level",
    label: "Level",
    width: "max-content",
    value: (line) => toLevelName(line.severity),
    cell: (line) => <Level level={toLevelName(line.severity)} />,
  },
  {
    id: "service",
    label: "Service",
    width: "minmax(6ch,16ch)",
    value: (line) => line.service,
    tone: () => "muted",
    cell: (line) => <ServiceName name={line.service} resource={line.resource} />,
  },
  { id: "body", label: "Message", width: "minmax(0,1fr)", value: (line) => line.body },
];

interface LogLinesProps {
  readonly lines: ReadonlyArray<LogLine>;
  readonly selectedKey: string | undefined;
  readonly onSelect: (line: LogLine | undefined) => void;
}

export default function LogLines(props: LogLinesProps) {
  return (
    <Table
      label="Log lines"
      rows={props.lines}
      columns={COLUMNS}
      selected={(line) => props.selectedKey === toLineKey(line)}
      tone={(line) => (line.severity >= MIN_ERROR_SEVERITY ? "error" : undefined)}
      onRowClick={(line) =>
        props.onSelect(props.selectedKey === toLineKey(line) ? undefined : line)
      }
    />
  );
}

export function toLineKey(line: LogLine): string {
  return `${line.logged_at} ${line.service} ${line.span_id ?? ""} ${line.body}`;
}
