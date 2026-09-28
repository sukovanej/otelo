import { For } from "solid-js";

import type { LogLine } from "@siner/api";
import { Level } from "@siner/ui";

import { closedErrorRow, closedRow, header, lineColumns, openRow, row } from "../classes";
import { toggleRow } from "../row";
import Service from "../Service";
import { formatTime, parseTime, timeWidth } from "../time";

/** What tells a line apart, so it stays selected when a reload brings it
 * again. */
export const lineKey = (line: LogLine) =>
  `${line.time} ${line.service} ${line.span_id ?? ""} ${line.body}`;

/** The lowest severity number of ERROR. */
const ERROR = 17;

/** Log lines, newest first. A click on a line selects it, and a click on the
 * selected one lets it go. */
export default function LogLines(props: {
  lines: LogLine[];
  /** The key of the selected line. */
  selected: string | undefined;
  onSelect: (line: LogLine | undefined) => void;
}) {
  return (
    <div
      class="font-mono text-sm"
      style={{ "--time-width": timeWidth(props.lines.map((line) => line.time)) }}
    >
      <div class={`${header} ${lineColumns}`} aria-hidden="true">
        <span>Time</span>
        <span>Level</span>
        <span>Service</span>
        <span>Message</span>
      </div>
      <div role="list">
        <For each={props.lines}>
          {(line) => {
            const key = lineKey(line);
            const isSelected = () => props.selected === key;
            const toggle = () => props.onSelect(isSelected() ? undefined : line);
            return (
              <div class="border-b border-line" role="listitem">
                <div
                  class={`${row} ${lineColumns} py-1.5 ${
                    isSelected() ? openRow : line.severity >= ERROR ? closedErrorRow : closedRow
                  }`}
                  aria-pressed={isSelected()}
                  {...toggleRow(toggle)}
                >
                  <time class="whitespace-nowrap text-muted" datetime={line.time}>
                    {formatTime(parseTime(line.time))}
                  </time>
                  <Level level={line.level} />
                  <span class="text-muted">
                    <Service name={line.service} resource={line.resource} />
                  </span>
                  <span class="truncate">{line.body}</span>
                </div>
              </div>
            );
          }}
        </For>
      </div>
    </div>
  );
}
