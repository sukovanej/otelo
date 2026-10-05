import { type JSX, Portal } from "@solidjs/web";
import { createEffect, createSignal, For, Show } from "solid-js";

import { formatInstant } from "../scale";
import type { Unit } from "../units";
import Value from "../Value";

const CROSSHAIR_GAP_PX = 12;
const PLOT_TOP_GAP_PX = 4;
const WINDOW_EDGE_GAP_PX = 8;

interface TooltipRow {
  readonly label: JSX.Element;
  readonly cssColor: string;
  readonly value: number | null;
}

interface ChartPanelTooltipProps {
  readonly plot: HTMLElement;
  readonly crosshairX: number;
  readonly bucketStartMs: number;
  readonly stepMs: number;
  readonly rows: ReadonlyArray<TooltipRow>;
  readonly total: number | undefined;
  readonly unit: Unit;
}

export default function ChartPanelTooltip(props: ChartPanelTooltipProps) {
  const [tip, setTip] = createSignal<HTMLDivElement>();

  createEffect(
    () => [tip(), props.plot, props.crosshairX, props.rows, props.total] as const,
    ([shownTip, plot, crosshairX]) => {
      if (shownTip) placeTipBesideCrosshair(shownTip, plot.getBoundingClientRect(), crosshairX);
    },
  );

  return (
    <Portal
      // In the open dialog around the chart, or else in the body, so no panel
      // clips the tip.
      mount={props.plot.closest("dialog[open]") ?? document.body}
    >
      <div
        ref={setTip}
        class="pointer-events-none fixed top-0 left-0 z-50 min-w-40 rounded-md border border-line bg-surface px-2.5 py-2 text-xs shadow-popup"
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
    </Portal>
  );
}

function placeTipBesideCrosshair(tip: HTMLElement, plotBox: DOMRect, crosshairX: number) {
  const { offsetWidth: width, offsetHeight: height } = tip;
  const windowWidth = document.documentElement.clientWidth;
  const anchorX = plotBox.left + crosshairX;
  const leftOfCrosshair = anchorX - CROSSHAIR_GAP_PX - width;
  const rightOfCrosshair = anchorX + CROSSHAIR_GAP_PX;
  const fitsLeft = leftOfCrosshair >= WINDOW_EDGE_GAP_PX;
  const fitsRight = rightOfCrosshair + width <= windowWidth - WINDOW_EDGE_GAP_PX;
  const prefersLeft = crosshairX > plotBox.width / 2;
  const opensLeft = prefersLeft ? fitsLeft || !fitsRight : fitsLeft && !fitsRight;
  const left = Math.min(
    opensLeft ? leftOfCrosshair : rightOfCrosshair,
    windowWidth - WINDOW_EDGE_GAP_PX - width,
  );
  const top = Math.min(
    plotBox.top + PLOT_TOP_GAP_PX,
    window.innerHeight - WINDOW_EDGE_GAP_PX - height,
  );
  tip.style.left = `${Math.max(left, WINDOW_EDGE_GAP_PX)}px`;
  tip.style.top = `${Math.max(top, WINDOW_EDGE_GAP_PX)}px`;
}
