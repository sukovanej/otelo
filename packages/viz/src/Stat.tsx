import { type JSX, Show } from "solid-js";

import type { SeriesColor } from "./color";
import Sparkline from "./Sparkline";
import type { Unit } from "./units";
import Value from "./Value";

/** A state that a stat shows in its color, with its label saying what the
 * number means, since a color alone does not. */
export type StatTone = "default" | "error" | "warn" | "success";

const tones: Record<StatTone, string> = {
  default: "",
  error: "text-error",
  warn: "text-warn",
  success: "text-success",
};

/**
 * One number that sums up a range, such as the requests or the error rate:
 * its label, the value in its unit, a line under it, and the shape of the
 * value over the range when `trend` is given.
 */
export default function Stat(props: {
  label: string;
  value: number | null | undefined;
  unit: Unit;
  /** A line under the value, such as a rate or a count it comes from. */
  detail?: JSX.Element;
  tone?: StatTone;
  /** The value in each step of the range, for a line under the number. */
  trend?: (number | null)[];
  trendColor?: SeriesColor;
}) {
  return (
    <div class="flex min-w-0 flex-col rounded-lg border border-line bg-panel px-4 pt-3 pb-3">
      <div class="truncate text-xs font-medium text-muted">{props.label}</div>
      <div class={`mt-1 text-[26px] leading-8 ${tones[props.tone ?? "default"]}`}>
        <Value value={props.value} unit={props.unit} />
      </div>
      <div class="mt-0.5 flex min-h-5 items-end justify-between gap-3">
        <span class="truncate text-xs text-muted">{props.detail}</span>
        <Show when={props.trend}>
          {(trend) => (
            <Sparkline
              series={[{ values: trend(), color: props.trendColor ?? "series-1" }]}
              kind="line"
              width={64}
              height={18}
            />
          )}
        </Show>
      </div>
    </div>
  );
}
