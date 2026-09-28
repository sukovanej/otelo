import { Button, Level } from "@siner/ui";
import { createSignal, For, Show } from "solid-js";
import type { LogGroup } from "@siner/api";
import Measure from "../Measure";
import { templateTerm } from "../query";
import { toggleRow } from "../row";
import { ago, formatTime, parseTime } from "../time";
import {
  body,
  closedRow,
  detail,
  groupColumns,
  header,
  heading,
  openRow,
  row,
  times,
} from "../classes";

/** A template with its placeholders marked, so the fixed words stand out. */
function Template(props: { text: string }) {
  const parts = () => props.text.split(/(<(?:num|str|uuid|hex)>)/);
  return (
    <For each={parts()}>
      {(part, i) => (i() % 2 === 1 ? <span class="text-placeholder">{part}</span> : part)}
    </For>
  );
}

/**
 * Log lines grouped by message template, the largest group first. A click
 * on a group opens its samples, and from there its lines.
 */
export default function LogGroupList(props: {
  groups: LogGroup[];
  onShowLines: (term: string) => void;
}) {
  const [open, setOpen] = createSignal<string>();
  const most = () => Math.max(1, ...props.groups.map((group) => group.count));

  return (
    <div class="font-mono text-sm">
      <div class={`${header} ${groupColumns}`} aria-hidden="true">
        <span class="text-right">Lines</span>
        <span>Level</span>
        <span>Template</span>
        <span>Services</span>
        <span class="text-right">Last</span>
      </div>
      <div role="list">
        <For each={props.groups}>
          {(group) => {
            const isOpen = () => open() === group.template;
            const term = templateTerm(group.template);
            const toggle = () => setOpen(isOpen() ? undefined : group.template);
            return (
              <div class="border-b border-line" role="listitem">
                <div
                  class={`${row} ${groupColumns} py-2 ${isOpen() ? openRow : closedRow}`}
                  aria-expanded={isOpen()}
                  {...toggleRow(toggle)}
                >
                  <Measure share={group.count / most()}>{group.count.toLocaleString()}</Measure>
                  <Level level={group.level} />
                  <span class="wrap-anywhere">
                    <Template text={group.template} />
                  </span>
                  <span class="truncate text-muted">{group.services.join(", ")}</span>
                  <time
                    class="whitespace-nowrap text-muted"
                    datetime={group.last}
                    title={group.last}
                  >
                    {ago(parseTime(group.last))}
                  </time>
                </div>
                <Show when={isOpen()}>
                  <div class={detail}>
                    <div class={times}>
                      From {formatTime(parseTime(group.first))} to{" "}
                      {formatTime(parseTime(group.last))}
                    </div>
                    <h3 class={heading}>Samples</h3>
                    <For each={group.samples}>{(sample) => <pre class={body}>{sample}</pre>}</For>
                    <Show when={term}>
                      {(shown) => (
                        <Button
                          class="mt-1.5"
                          title={`Add ${shown()} to the query`}
                          onClick={() => props.onShowLines(shown())}
                        >
                          Show lines
                        </Button>
                      )}
                    </Show>
                  </div>
                </Show>
              </div>
            );
          }}
        </For>
      </div>
    </div>
  );
}
