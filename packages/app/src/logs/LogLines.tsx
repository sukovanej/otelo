import type { LogLine } from "@siner/api";
import { Level } from "@siner/ui";
import { type Column, Table } from "@siner/viz";

import Service from "../Service";
import { formatTime, parseTime } from "../time";
import { levelName } from "./level";

/** What tells a line apart, so it stays selected when a reload brings it
 * again. */
export const lineKey = (line: LogLine) =>
  `${line.logged_at} ${line.service} ${line.span_id ?? ""} ${line.body}`;

/** The lowest severity number of ERROR. */
const ERROR = 17;

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
    value: (line) => levelName(line.severity),
    cell: (line) => <Level level={levelName(line.severity)} />,
  },
  {
    id: "service",
    label: "Service",
    width: "minmax(6ch,16ch)",
    value: (line) => line.service,
    tone: () => "muted",
    cell: (line) => <Service name={line.service} resource={line.resource} />,
  },
  { id: "body", label: "Message", width: "minmax(0,1fr)", value: (line) => line.body },
];

/** Log lines, newest first. A click on a line selects it, and a click on the
 * selected one lets it go. */
export default function LogLines(props: {
  lines: LogLine[];
  /** The key of the selected line. */
  selected: string | undefined;
  onSelect: (line: LogLine | undefined) => void;
}) {
  return (
    <Table
      label="Log lines"
      rows={props.lines}
      columns={COLUMNS}
      selected={(line) => props.selected === lineKey(line)}
      tone={(line) => (line.severity >= ERROR ? "error" : undefined)}
      onRowClick={(line) => props.onSelect(props.selected === lineKey(line) ? undefined : line)}
    />
  );
}
