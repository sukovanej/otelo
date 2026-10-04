import type { JSX } from "@solidjs/web";
import { For } from "solid-js";

import { pickDefaultColor, toCssColor } from "../color";
import type { ChartKind, TimeSeries } from "../series";

const KEY_CLASSES: Record<ChartKind, string> = {
  bar: "size-2.5 rounded-[3px]",
  line: "h-0.5 w-3 rounded-full",
  area: "h-0.5 w-3 rounded-full",
};

interface ChartPanelLegendProps {
  readonly series: ReadonlyArray<TimeSeries>;
  readonly kind: ChartKind;
  readonly isolatedIndex: number | undefined;
  readonly drawLabel: (index: number) => JSX.Element;
  readonly onIsolate: (index: number | undefined) => void;
}

export default function ChartPanelLegend(props: ChartPanelLegendProps) {
  return (
    <ul class="m-0 flex list-none flex-wrap justify-end gap-x-1 p-0 text-xs text-muted">
      <For each={props.series}>
        {(series, index) => {
          const cssColor = () => toCssColor(series.color ?? pickDefaultColor(index()));
          const isIsolated = () => props.isolatedIndex === index();
          return (
            <li>
              <button
                type="button"
                class={[
                  "flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 hover:bg-hover hover:text-ink",
                  { "opacity-40": props.isolatedIndex !== undefined && !isIsolated() },
                ]}
                aria-pressed={isIsolated() ? "true" : "false"}
                title={isIsolated() ? "Show every series" : `Show ${series.label} alone`}
                onClick={() => props.onIsolate(isIsolated() ? undefined : index())}
              >
                <span class={KEY_CLASSES[props.kind]} style={{ background: cssColor() }} />
                {props.drawLabel(index())}
              </button>
            </li>
          );
        }}
      </For>
    </ul>
  );
}
