import { createMemo, For, Show } from "solid-js";

import { type SeriesColor, toCssColor } from "./color";

const DEFAULT_WIDTH_PX = 120;
const DEFAULT_HEIGHT_PX = 24;

interface SparklineSeries {
  readonly values: ReadonlyArray<number | null>;
  readonly color: SeriesColor;
}

type SparklineProps = SparklineBarsProps | SparklineLineProps;

interface SparklineSize {
  readonly width?: number;
  readonly height?: number;
}

interface SparklineBarsProps extends SparklineSize {
  readonly kind: "bar";
  readonly series: ReadonlyArray<SparklineSeries>;
}

interface SparklineLineProps extends SparklineSize {
  readonly kind: "line";
  readonly values: ReadonlyArray<number | null>;
  readonly color: SeriesColor;
}

export default function Sparkline(props: SparklineProps) {
  const width = () => props.width ?? DEFAULT_WIDTH_PX;
  const height = () => props.height ?? DEFAULT_HEIGHT_PX;
  const barRects = createMemo(() =>
    props.kind === "bar" ? stackBarRects(props.series, width(), height()) : [],
  );
  const linePath = createMemo(() =>
    props.kind === "line" ? toLinePath(props.values, props.color, width(), height()) : undefined,
  );

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
      <For each={barRects()} keyed={false}>
        {(bar) => (
          <rect
            x={bar().x}
            y={bar().y}
            width={bar().width}
            height={bar().height}
            fill={bar().cssColor}
          />
        )}
      </For>
      <Show when={linePath()}>
        {(line) => (
          <path
            d={line().path}
            fill="none"
            stroke={line().cssColor}
            stroke-width="1.5"
            stroke-linejoin="round"
            stroke-linecap="round"
          />
        )}
      </Show>
    </svg>
  );
}

interface BarRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly cssColor: string;
}

function stackBarRects(
  stackedSeries: ReadonlyArray<SparklineSeries>,
  width: number,
  height: number,
): BarRect[] {
  const bucketCount = Math.max(1, ...stackedSeries.map((series) => series.values.length));
  const bucketWidth = width / bucketCount;
  const barWidth = Math.max(1, bucketWidth - Math.min(1, bucketWidth / 3));
  const bucketTotals = Array.from({ length: bucketCount }, (_, bucketIndex) =>
    stackedSeries.reduce((sum, series) => sum + (series.values[bucketIndex] ?? 0), 0),
  );
  const largestTotal = Math.max(0, ...bucketTotals);
  return bucketTotals.flatMap((_, bucketIndex) => {
    let stackHeight = 0;
    return stackedSeries.flatMap((series) => {
      const value = series.values[bucketIndex] ?? 0;
      if (value <= 0) return [];
      const barHeight = Math.max(1, scaleToHeight(value, largestTotal, height));
      const rect = {
        x: bucketIndex * bucketWidth,
        y: height - stackHeight - barHeight,
        width: barWidth,
        height: barHeight,
        cssColor: toCssColor(series.color),
      };
      stackHeight += barHeight;
      return [rect];
    });
  });
}

interface LinePath {
  readonly path: string;
  readonly cssColor: string;
}

function toLinePath(
  values: ReadonlyArray<number | null>,
  color: SeriesColor,
  width: number,
  height: number,
): LinePath {
  const bucketWidth = width / Math.max(1, values.length);
  const largestValue = Math.max(0, ...values.map((value) => value ?? 0));
  const points = values.flatMap((value, bucketIndex) =>
    value === null
      ? []
      : [
          `${(bucketIndex + 0.5) * bucketWidth},${height - 1 - scaleToHeight(value, largestValue, height)}`,
        ],
  );
  return { path: points.length > 0 ? `M${points.join("L")}` : "", cssColor: toCssColor(color) };
}

function scaleToHeight(value: number, largestValue: number, height: number): number {
  return largestValue > 0 ? (value / largestValue) * (height - 1) : 0;
}
