import { createSignal, For, Show } from "solid-js";

import type { LogGroup } from "@otelo/api";
import { Button, Level } from "@otelo/ui";
import { type Column, Table } from "@otelo/viz";

import { body, detail, heading, times } from "../classes";
import { templateTerm } from "../query";
import { ago, formatTime, parseTime } from "../time";
import { levelName } from "./level";

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
  const columns: Column<LogGroup>[] = [
    {
      id: "count",
      label: "Lines",
      unit: "count",
      meter: true,
      width: "16ch",
      value: (g) => g.count,
    },
    {
      id: "level",
      label: "Level",
      width: "max-content",
      value: (g) => levelName(g.severity),
      cell: (g) => <Level level={levelName(g.severity)} />,
    },
    {
      id: "template",
      label: "Template",
      width: "minmax(0,1fr)",
      value: (g) => g.template,
      cell: (g) => (
        <span class="wrap-anywhere">
          <Template text={g.template} />
        </span>
      ),
    },
    {
      id: "services",
      label: "Services",
      width: "minmax(8ch,20ch)",
      value: (g) => g.services.join(", "),
      tone: () => "muted",
    },
    {
      id: "last",
      label: "Last",
      align: "end",
      width: "max-content",
      value: (g) => g.last_at,
      tone: () => "muted",
      cell: (g) => (
        <time class="whitespace-nowrap" datetime={g.last_at} title={g.last_at}>
          {ago(parseTime(g.last_at))}
        </time>
      ),
    },
  ];

  return (
    <Table
      label="Log templates"
      rows={props.groups}
      columns={columns}
      selected={(g) => open() === g.template}
      onRowClick={(g) => setOpen(open() === g.template ? undefined : g.template)}
      expanded={(g) => open() === g.template}
      detail={(g) => <Samples group={g} onShowLines={props.onShowLines} />}
    />
  );
}

/** The samples of an open group, and a button that shows its lines. */
function Samples(props: { group: LogGroup; onShowLines: (term: string) => void }) {
  const term = () => templateTerm(props.group.template);
  return (
    <div class={detail}>
      <div class={times}>
        From {formatTime(parseTime(props.group.first_at))} to{" "}
        {formatTime(parseTime(props.group.last_at))}
      </div>
      <h3 class={heading}>Samples</h3>
      <For each={props.group.samples}>{(sample) => <pre class={body}>{sample}</pre>}</For>
      <Show when={term()}>
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
  );
}
