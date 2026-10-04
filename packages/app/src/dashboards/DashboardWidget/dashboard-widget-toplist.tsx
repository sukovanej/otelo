import { createMemo, For } from "solid-js";

import { Meter, Value } from "@otelo/viz";

import type { MeasuredQuery } from "../measure";
import DashboardWidgetGroupTitle from "./dashboard-widget-group-title";

interface DashboardWidgetToplistProps {
  readonly measured: MeasuredQuery;
}

export default function DashboardWidgetToplist(props: DashboardWidgetToplistProps) {
  const largestTotal = createMemo(() =>
    Math.max(0, ...props.measured.groups.map((group) => group.total ?? 0)),
  );
  return (
    <ol class="m-0 -mx-1 grid min-h-0 flex-1 list-none auto-rows-min grid-cols-[minmax(0,1fr)_minmax(8ch,0.5fr)_auto] overflow-y-auto p-0">
      <For
        each={props.measured.groups}
        keyed={false}
        fallback={
          <li class="col-span-full py-8 text-center text-muted">Nothing in this range matches.</li>
        }
      >
        {(group) => (
          <li class="col-span-full grid grid-cols-subgrid items-center gap-x-3 border-b border-line px-1 py-1.5 text-sm last:border-b-0">
            <span class="min-w-0 font-mono">
              <DashboardWidgetGroupTitle label={group().label} groupKey={group().groupKey} />
            </span>
            <Meter share={largestTotal() > 0 ? (group().total ?? 0) / largestTotal() : 0} />
            <span class="text-right">
              <Value value={group().total} unit={props.measured.unit} inColumn />
            </span>
          </li>
        )}
      </For>
    </ol>
  );
}
