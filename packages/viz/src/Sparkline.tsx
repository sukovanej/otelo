import { createMemo, For } from "solid-js";

import { cssColor, defaultColor } from "./color";
import type { TimeSeries } from "./TimeSeriesChart";

/**
 * The shape of values over time, small enough for a table cell or a stat:
 * stacked bars, or a line of the first series. It has no axes and no tip,
 * so the numbers it sums up have to be beside it.
 */
export default function Sparkline(props: {
  series: Pick<TimeSeries, "values" | "color">[];
  kind: "bar" | "line";
  width?: number;
  height?: number;
}) {
  const width = () => props.width ?? 120;
  const height = () => props.height ?? 24;
  const colored = createMemo(() =>
    props.series.map((s, i) => ({ values: s.values, css: cssColor(s.color ?? defaultColor(i)) })),
  );
  const count = () => Math.max(1, ...colored().map((s) => s.values.length));
  const slot = () => width() / count();
  const totals = createMemo(() =>
    Array.from({ length: count() }, (_, i) =>
      colored().reduce((sum, s) => sum + (s.values[i] ?? 0), 0),
    ),
  );
  const most = createMemo(() =>
    props.kind === "bar"
      ? Math.max(0, ...totals())
      : Math.max(0, ...(colored()[0]?.values.map((v) => v ?? 0) ?? [])),
  );
  const scale = (value: number) => (most() > 0 ? (value / most()) * (height() - 1) : 0);

  const bars = createMemo(() => {
    const w = Math.max(1, slot() - Math.min(1, slot() / 3));
    return Array.from({ length: count() }, (_, i) => {
      let base = 0;
      return colored().flatMap((s) => {
        const value = s.values[i] ?? 0;
        if (value <= 0) return [];
        const h = Math.max(1, scale(value));
        const rect = { x: i * slot(), y: height() - base - h, w, h, css: s.css };
        base += h;
        return [rect];
      });
    }).flat();
  });
  const line = createMemo(() => {
    const first = colored()[0];
    if (!first) return { d: "", css: "" };
    const points = first.values.flatMap((v, i) =>
      v === null ? [] : [`${(i + 0.5) * slot()},${height() - 1 - scale(v)}`],
    );
    return { d: points.length > 0 ? `M${points.join("L")}` : "", css: first.css };
  });

  return (
    <svg
      width={width()}
      height={height()}
      class="block shrink-0 overflow-visible"
      aria-hidden="true"
    >
      <line
        x1="0"
        x2={width()}
        y1={height() - 0.5}
        y2={height() - 0.5}
        stroke="var(--color-line)"
        stroke-width="1"
      />
      <For each={props.kind === "bar" ? bars() : []}>
        {(bar) => <rect x={bar.x} y={bar.y} width={bar.w} height={bar.h} fill={bar.css} />}
      </For>
      <path
        d={props.kind === "line" ? line().d : ""}
        fill="none"
        stroke={line().css}
        stroke-width="1.5"
        stroke-linejoin="round"
        stroke-linecap="round"
      />
    </svg>
  );
}
