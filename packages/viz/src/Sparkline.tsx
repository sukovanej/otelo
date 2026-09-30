import { createMemo, For } from "solid-js";

import { type SeriesColor, toCssColor } from "./color";

type SparklineKind = "bar" | "line";

interface SparklineSeries {
  readonly values: ReadonlyArray<number | null>;
  readonly color: SeriesColor;
}

interface SparklineProps {
  readonly series: ReadonlyArray<SparklineSeries>;
  readonly kind: SparklineKind;
  readonly width?: number;
  readonly height?: number;
}

export default function Sparkline(props: SparklineProps) {
  const width = () => props.width ?? 120;
  const height = () => props.height ?? 24;
  const coloredSeries = createMemo(() =>
    props.series.map((series) => ({
      values: series.values,
      cssColor: toCssColor(series.color),
    })),
  );
  const bucketCount = () => Math.max(1, ...coloredSeries().map((series) => series.values.length));
  const bucketWidth = () => width() / bucketCount();
  const bucketTotals = createMemo(() =>
    Array.from({ length: bucketCount() }, (_, bucketIndex) =>
      coloredSeries().reduce((sum, series) => sum + (series.values[bucketIndex] ?? 0), 0),
    ),
  );
  const largestValue = createMemo(() =>
    props.kind === "bar"
      ? Math.max(0, ...bucketTotals())
      : Math.max(0, ...(coloredSeries()[0]?.values.map((value) => value ?? 0) ?? [])),
  );
  const scaleToHeight = (value: number) =>
    largestValue() > 0 ? (value / largestValue()) * (height() - 1) : 0;

  const barRects = createMemo(() => {
    const barWidth = Math.max(1, bucketWidth() - Math.min(1, bucketWidth() / 3));
    return Array.from({ length: bucketCount() }, (_, bucketIndex) => {
      let stackHeight = 0;
      return coloredSeries().flatMap((series) => {
        const value = series.values[bucketIndex] ?? 0;
        if (value <= 0) return [];
        const barHeight = Math.max(1, scaleToHeight(value));
        const rect = {
          x: bucketIndex * bucketWidth(),
          y: height() - stackHeight - barHeight,
          width: barWidth,
          height: barHeight,
          cssColor: series.cssColor,
        };
        stackHeight += barHeight;
        return [rect];
      });
    }).flat();
  });
  const linePath = createMemo(() => {
    const first = coloredSeries()[0];
    if (!first) return { path: "", cssColor: "" };
    const points = first.values.flatMap((value, bucketIndex) =>
      value === null
        ? []
        : [`${(bucketIndex + 0.5) * bucketWidth()},${height() - 1 - scaleToHeight(value)}`],
    );
    return { path: points.length > 0 ? `M${points.join("L")}` : "", cssColor: first.cssColor };
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
      <For each={props.kind === "bar" ? barRects() : []}>
        {(bar) => (
          <rect x={bar.x} y={bar.y} width={bar.width} height={bar.height} fill={bar.cssColor} />
        )}
      </For>
      <path
        d={props.kind === "line" ? linePath().path : ""}
        fill="none"
        stroke={linePath().cssColor}
        stroke-width="1.5"
        stroke-linejoin="round"
        stroke-linecap="round"
      />
    </svg>
  );
}
