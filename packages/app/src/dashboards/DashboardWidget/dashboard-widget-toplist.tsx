import { createMemo, For } from "solid-js";

import { Value } from "@otelo/viz";

import type { MeasuredQuery } from "../measure";

interface DashboardWidgetToplistProps {
  readonly measured: MeasuredQuery;
}

export default function DashboardWidgetToplist(props: DashboardWidgetToplistProps) {
  const largestTotal = createMemo(() =>
    Math.max(0, ...props.measured.groups.map((group) => group.total ?? 0)),
  );
  return (
    <ol class="m-0 -mx-1 flex min-h-0 flex-1 list-none flex-col gap-1 overflow-y-auto p-0">
      <For
        each={props.measured.groups}
        keyed={false}
        fallback={<li class="py-8 text-center text-muted">Nothing in this range matches.</li>}
      >
        {(group) => (
          <li class="relative flex items-center gap-3 px-1 py-0.5 text-sm">
            <span
              class="absolute inset-y-0 left-0 rounded bg-bar"
              style={{
                width: `${largestTotal() > 0 ? ((group().total ?? 0) / largestTotal()) * 100 : 0}%`,
              }}
            />
            <span class="relative min-w-0 flex-1 truncate font-mono" title={group().label}>
              {group().label}
            </span>
            <span class="relative">
              <Value value={group().total} unit={props.measured.unit} inColumn />
            </span>
          </li>
        )}
      </For>
    </ol>
  );
}
