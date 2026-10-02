import { For, Show } from "solid-js";

import { formatInstant } from "../scale";
import type { Unit } from "../units";
import Value from "../Value";

interface TooltipRow {
  readonly label: string;
  readonly cssColor: string;
  readonly value: number | null;
}

interface ChartPanelTooltipProps {
  readonly crosshairX: number;
  readonly chartWidth: number;
  readonly bucketStartMs: number;
  readonly stepMs: number;
  readonly rows: ReadonlyArray<TooltipRow>;
  readonly total: number | undefined;
  readonly unit: Unit;
}

export default function ChartPanelTooltip(props: ChartPanelTooltipProps) {
  return (
    <div
      class="pointer-events-none absolute top-1 z-10 min-w-40 rounded-md border border-line bg-surface px-2.5 py-2 text-xs shadow-popup"
      style={
        props.crosshairX > props.chartWidth / 2
          ? { right: `${props.chartWidth - props.crosshairX + 12}px` }
          : { left: `${props.crosshairX + 12}px` }
      }
    >
      <div class="mb-1.5 whitespace-nowrap text-muted tabular-nums">
        {formatInstant(props.bucketStartMs)} – {formatInstant(props.bucketStartMs + props.stepMs)}
      </div>
      <table class="border-collapse">
        <tbody>
          <For each={props.rows} keyed={false}>
            {(row) => (
              <tr>
                <td class="py-px pr-2">
                  <span
                    class="block h-0.5 w-3 rounded-full"
                    style={{ background: row().cssColor }}
                  />
                </td>
                <td class="py-px pr-3 text-right">
                  <Value value={row().value} unit={props.unit} />
                </td>
                <td class="py-px whitespace-nowrap text-muted">{row().label}</td>
              </tr>
            )}
          </For>
          <Show when={props.total !== undefined}>
            <tr class="border-t border-line">
              <td />
              <td class="pt-1 pr-3 text-right">
                <Value value={props.total} unit={props.unit} />
              </td>
              <td class="pt-1 text-muted">Total</td>
            </tr>
          </Show>
        </tbody>
      </table>
    </div>
  );
}
