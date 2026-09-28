import { Level } from "@siner/ui";
import { For } from "solid-js";
import type { LogLine } from "../api";
import { formatTime, parseTime } from "../time";
import { closedErrorRow, closedRow, header, lineColumns, openRow, row } from "./classes";

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
  // Lines from before today show the date too.
  const timeWidth = () => {
    const now = new Date();
    return props.lines.some((line) => formatTime(parseTime(line.time), now).length > 12)
      ? "18ch"
      : "12ch";
  };

  return (
    <div class="font-mono text-sm" style={{ "--time-width": timeWidth() }}>
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
                  role="button"
                  tabIndex={0}
                  aria-pressed={isSelected()}
                  onClick={() => {
                    // A click that ends a text selection is not a toggle.
                    if (window.getSelection()?.isCollapsed !== false) toggle();
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      toggle();
                    }
                  }}
                >
                  <time class="whitespace-nowrap text-muted" datetime={line.time}>
                    {formatTime(parseTime(line.time))}
                  </time>
                  <Level level={line.level} />
                  <span class="truncate text-muted">{line.service}</span>
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
