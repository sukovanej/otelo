import { Level } from "@siner/ui";
import { createSignal, For, Show } from "solid-js";
import type { LogLine } from "../api";
import { formatTime, parseTime } from "../time";
import { closedRow, openRow, row } from "./classes";
import Fields from "./Fields";

const lineKey = (line: LogLine) =>
  `${line.time} ${line.service} ${line.span_id ?? ""} ${line.body}`;

/** Log lines, newest first. A click on a line opens all of it. */
export default function LogLines(props: { lines: LogLine[]; onFilter: (term: string) => void }) {
  // Keyed by content, so an open line stays open when a reload brings it again.
  const [open, setOpen] = createSignal<ReadonlySet<string>>(new Set());
  const toggle = (key: string) =>
    setOpen((keys) => {
      const next = new Set(keys);
      if (!next.delete(key)) next.add(key);
      return next;
    });

  // Lines from before today show the date too.
  const timeWidth = () => {
    const now = new Date();
    return props.lines.some((line) => formatTime(parseTime(line.time), now).length > 12)
      ? "18ch"
      : "12ch";
  };

  return (
    <div class="font-mono text-sm" role="list" style={{ "--time-width": timeWidth() }}>
      <For each={props.lines}>
        {(line) => {
          const key = lineKey(line);
          const isOpen = () => open().has(key);
          return (
            <div class="border-b border-line" role="listitem">
              <div
                class={`${row} grid-cols-[var(--time-width)_5ch_minmax(6ch,16ch)_1fr] py-1.5 ${isOpen() ? openRow : closedRow}`}
                role="button"
                tabIndex={0}
                aria-expanded={isOpen()}
                onClick={() => {
                  // A click that ends a text selection is not a toggle.
                  if (window.getSelection()?.isCollapsed !== false) toggle(key);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    toggle(key);
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
              <Show when={isOpen()}>
                <Fields line={line} onFilter={props.onFilter} />
              </Show>
            </div>
          );
        }}
      </For>
    </div>
  );
}
