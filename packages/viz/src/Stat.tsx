import type { JSX } from "@solidjs/web";
import { Show } from "solid-js";

import type { SeriesColor } from "./color";
import Sparkline from "./Sparkline";
import type { Unit } from "./units";
import Value from "./Value";

const STAT_TONE_CLASSES: Record<StatTone, string> = {
  error: "text-error",
  warn: "text-warn",
  success: "text-success",
};

type StatTone = "error" | "warn" | "success";

interface StatProps {
  readonly label: string;
  readonly value: number | null | undefined;
  readonly unit: Unit;
  readonly detail?: JSX.Element | undefined;
  readonly tone?: StatTone | undefined;
  readonly trend?: ReadonlyArray<number | null> | undefined;
  readonly trendColor?: SeriesColor | undefined;
}

export default function Stat(props: StatProps) {
  return (
    <div class="flex min-w-0 flex-col rounded-lg border border-line bg-panel px-4 pt-3 pb-3">
      <div class="truncate text-xs font-medium text-muted">{props.label}</div>
      <div class={`mt-1 text-[26px] leading-8 ${props.tone ? STAT_TONE_CLASSES[props.tone] : ""}`}>
        <Value value={props.value} unit={props.unit} />
      </div>
      <div class="mt-0.5 flex min-h-5 items-end justify-between gap-3">
        <span class="truncate text-xs text-muted">{props.detail}</span>
        <Show when={props.trend}>
          {(trend) => (
            <Sparkline
              kind="line"
              values={trend()}
              color={props.trendColor ?? "series-1"}
              width={64}
              height={18}
            />
          )}
        </Show>
      </div>
    </div>
  );
}
