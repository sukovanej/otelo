import type { LogLine } from "@otelo/api";
import { LevelBadge } from "@otelo/ui";
import { type Column, Table } from "@otelo/viz";

import ServiceName from "../ServiceName";
import { formatTime, parseTime } from "../time";
import { toLevelName } from "./level";

const MIN_ERROR_SEVERITY = 17;

const COLUMNS: Column<LogLine>[] = [
  {
    kind: "cell",
    id: "time",
    label: "Time",
    width: "max-content",
    tone: () => "muted",
    cell: (line) => (
      <time class="whitespace-nowrap" datetime={line.logged_at}>
        {formatTime(parseTime(line.logged_at))}
      </time>
    ),
  },
  {
    kind: "cell",
    id: "level",
    label: "Level",
    width: "max-content",
    cell: (line) => <LevelBadge level={toLevelName(line.severity)} />,
  },
  {
    kind: "cell",
    id: "service",
    label: "Service",
    width: "minmax(6ch,16ch)",
    tone: () => "muted",
    cell: (line) => <ServiceName name={line.service} resource={line.resource} />,
  },
  {
    kind: "text",
    id: "body",
    label: "Message",
    width: "minmax(0,1fr)",
    value: (line) => line.body,
  },
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
      rowKey={toLineKey}
      columns={COLUMNS}
      selectedKey={() => props.selectedKey}
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
