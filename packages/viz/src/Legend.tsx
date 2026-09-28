import { For, Show } from "solid-js";

import { cssColor, defaultColor } from "./color";
import type { ChartKind, TimeSeries } from "./TimeSeriesChart";

/**
 * The names of the series of a chart, each after a key of its color: a
 * square for bars, a short line for lines. A click on a name shows that
 * series alone, which `isolated` holds by its place in the list, and a
 * second click shows them all again.
 */
export default function Legend(props: {
  series: Pick<TimeSeries, "label" | "color">[];
  kind: ChartKind;
  isolated: number | undefined;
  onIsolate: (index: number | undefined) => void;
}) {
  return (
    <ul class="m-0 flex list-none flex-wrap justify-end gap-x-1 p-0 text-xs text-muted">
      <For each={props.series}>
        {(s, i) => {
          const css = () => cssColor(s.color ?? defaultColor(i()));
          const alone = () => props.isolated === i();
          return (
            <li>
              <button
                type="button"
                class="flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 hover:bg-hover hover:text-ink"
                classList={{ "opacity-40": props.isolated !== undefined && !alone() }}
                aria-pressed={alone()}
                title={alone() ? "Show every series" : `Show ${s.label} alone`}
                onClick={() => props.onIsolate(alone() ? undefined : i())}
              >
                <Show
                  when={props.kind === "bar"}
                  fallback={<span class="h-0.5 w-3 rounded-full" style={{ background: css() }} />}
                >
                  <span class="size-2.5 rounded-[3px]" style={{ background: css() }} />
                </Show>
                {s.label}
              </button>
            </li>
          );
        }}
      </For>
    </ul>
  );
}
