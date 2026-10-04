import { Show } from "solid-js";

import { Sparkline, Value } from "@otelo/viz";

import type { MeasuredQuery } from "../measure";

interface DashboardWidgetValueProps {
  readonly measured: MeasuredQuery;
}

export default function DashboardWidgetValue(props: DashboardWidgetValueProps) {
  const group = () => props.measured.groups[0];
  return (
    <div class="@container flex flex-1 flex-col items-center justify-center gap-3">
      <div class="text-[26px] leading-none @[13rem]:text-[40px]">
        <Value value={group()?.total} unit={props.measured.unit} />
      </div>
      <Show when={group()}>
        {(shownGroup) => (
          <Sparkline
            kind="line"
            values={shownGroup().values}
            color={shownGroup().color ?? "series-1"}
            width={120}
            height={28}
          />
        )}
      </Show>
    </div>
  );
}
