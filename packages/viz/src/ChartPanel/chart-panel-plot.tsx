import { createMemo, createSignal, flush, For, onSettled, Show } from "solid-js";

import { pickDefaultColor, toCssColor } from "../color";
import { formatInstant, formatTick, pickTimeTicks, pickValueTicks } from "../scale";
import type { ChartKind, TimeFrame, TimeSeries } from "../series";
import { formatValue, type Unit } from "../units";
import ChartPanelTooltip from "./chart-panel-tooltip";

const PLOT_HEIGHT_PX = 150;
const MIN_FILLED_PLOT_HEIGHT_PX = 40;
const AXIS_HEIGHT_PX = 20;
const PLOT_LEFT_PX = 52;
const PLOT_RIGHT_MARGIN_PX = 4;
const PLOT_TOP_PX = 8;
const BAR_GAP_PX = 2;
const BAR_CORNER_RADIUS_PX = 3;
const MAX_BAR_WIDTH_PX = 24;
const MIN_ROUNDED_BAR_WIDTH_PX = 6;
const LONE_VALUE_DASH_WIDTH_PX = 6;
const MIN_ZOOM_DRAG_PX = 4;

interface ColoredSeries extends TimeSeries {
  readonly cssColor: string;
  readonly index: number;
}

interface BarPath {
  readonly cssColor: string;
  readonly path: string;
}

interface LineRun {
  readonly cssColor: string;
  readonly line: string;
  readonly area: string;
}

interface HoveredPoint {
  readonly cssColor: string;
  readonly value: number;
}

interface ZoomDrag {
  readonly fromX: number;
  readonly toX: number;
}

type PlotHeight = "fixed" | "fill";

interface ChartPanelPlotProps {
  readonly label: string;
  readonly frame: TimeFrame;
  readonly series: ReadonlyArray<TimeSeries>;
  readonly kind: ChartKind;
  readonly unit: Unit;
  readonly isolatedIndex: number | undefined;
  readonly loading: boolean;
  readonly emptyMessage: string | undefined;
  readonly height: PlotHeight;
  readonly onZoom: (startMs: number, endMs: number) => void;
}

export default function ChartPanelPlot(props: ChartPanelPlotProps) {
  let plotElement!: HTMLDivElement;
  const [chartWidth, setChartWidth] = createSignal(0);
  const [filledHeight, setFilledHeight] = createSignal(0);
  onSettled(() => {
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      setChartWidth(entry.contentRect.width);
      setFilledHeight(entry.contentRect.height);
    });
    observer.observe(plotElement);
    return () => observer.disconnect();
  });

  const plotHeight = () =>
    props.height === "fill"
      ? Math.max(MIN_FILLED_PLOT_HEIGHT_PX, filledHeight() - PLOT_TOP_PX - AXIS_HEIGHT_PX)
      : PLOT_HEIGHT_PX;
  const canvasHeight = () => PLOT_TOP_PX + plotHeight() + AXIS_HEIGHT_PX;
  const plotWidth = () => Math.max(40, chartWidth() - PLOT_LEFT_PX - PLOT_RIGHT_MARGIN_PX);
  const frame = createMemo(() => props.frame);
  const frameLengthMs = () => Math.max(1, frame().endMs - frame().startMs);
  const timeToX = (timeMs: number) =>
    PLOT_LEFT_PX + ((timeMs - frame().startMs) / frameLengthMs()) * plotWidth();
  const xToTimeMs = (x: number) =>
    frame().startMs + ((x - PLOT_LEFT_PX) / plotWidth()) * frameLengthMs();
  const bucketCenterX = (bucketIndex: number) =>
    timeToX((frame().bucketStartsMs[bucketIndex] ?? frame().startMs) + frame().stepMs / 2);

  const coloredSeries = createMemo<ColoredSeries[]>(() =>
    props.series.map((series, index) => ({
      ...series,
      cssColor: toCssColor(series.color ?? pickDefaultColor(index)),
      index,
    })),
  );
  const shownSeries = createMemo(() => {
    const isolatedIndex = props.isolatedIndex;
    return isolatedIndex === undefined || isolatedIndex >= coloredSeries().length
      ? coloredSeries()
      : coloredSeries().filter((series) => series.index === isolatedIndex);
  });
  const hasNoValues = createMemo(() =>
    coloredSeries().every((series) => series.values.every((value) => value === null)),
  );

  const bucketTops = createMemo(() =>
    frame().bucketStartsMs.map((_, bucketIndex) => {
      const values = shownSeries().map((series) => series.values[bucketIndex] ?? 0);
      return props.kind === "bar"
        ? values.reduce((sum, value) => sum + value, 0)
        : Math.max(0, ...values);
    }),
  );
  const yTicks = createMemo(() => pickValueTicks(Math.max(0, ...bucketTops())), {
    equals: haveSameItems,
  });
  const yAxisMax = createMemo(() => yTicks().at(-1) || 1);
  const valueToY = (value: number) =>
    PLOT_TOP_PX + plotHeight() - (value / yAxisMax()) * plotHeight();
  const xTicks = createMemo(
    () => pickTimeTicks(frame().startMs, frame().endMs, Math.floor(plotWidth() / 96)),
    { equals: haveSameItems },
  );

  const barWidth = createMemo(() => {
    const bucketWidth = (frame().stepMs / frameLengthMs()) * plotWidth();
    return Math.max(
      1,
      Math.min(MAX_BAR_WIDTH_PX, bucketWidth - Math.min(BAR_GAP_PX, bucketWidth / 3)),
    );
  });

  const barPaths = createMemo<BarPath[]>(
    () =>
      props.kind !== "bar"
        ? []
        : frame().bucketStartsMs.flatMap((_, bucketIndex) => {
            const width = barWidth();
            const left = bucketCenterX(bucketIndex) - width / 2;
            let stackedValue = 0;
            const stack = shownSeries().flatMap((series) => {
              const value = series.values[bucketIndex] ?? 0;
              const segment = {
                cssColor: series.cssColor,
                fromValue: stackedValue,
                toValue: stackedValue + value,
              };
              stackedValue += value;
              return value > 0 ? [segment] : [];
            });
            return stack.map((segment, segmentIndex) => {
              const isTopSegment = segmentIndex === stack.length - 1;
              const bottom = valueToY(segment.fromValue) - (segmentIndex > 0 ? BAR_GAP_PX / 2 : 0);
              const top = valueToY(segment.toValue) + (isTopSegment ? 0 : BAR_GAP_PX / 2);
              const height = Math.max(1, bottom - top);
              return {
                cssColor: segment.cssColor,
                path: toBarPath(
                  left,
                  bottom - height,
                  width,
                  height,
                  isTopSegment && width >= MIN_ROUNDED_BAR_WIDTH_PX,
                ),
              };
            });
          }),
    { equals: (previous, next) => haveSameItems(previous, next, isSameBarPath) },
  );

  const lineRuns = createMemo<LineRun[]>(
    () =>
      props.kind === "bar"
        ? []
        : shownSeries().flatMap((series) =>
            splitIntoRuns(series.values).map((run) => {
              const points = run.map(
                ({ bucketIndex, value }) => `${bucketCenterX(bucketIndex)},${valueToY(value)}`,
              );
              const firstBucketIndex = run[0]?.bucketIndex ?? 0;
              const lastBucketIndex = run.at(-1)?.bucketIndex ?? firstBucketIndex;
              const loneValue = run.length === 1 ? run[0] : undefined;
              return {
                cssColor: series.cssColor,
                line: loneValue
                  ? toLoneValueDash(bucketCenterX(loneValue.bucketIndex), valueToY(loneValue.value))
                  : `M${points.join("L")}`,
                area: `M${bucketCenterX(firstBucketIndex)},${valueToY(0)}L${points.join("L")}L${bucketCenterX(lastBucketIndex)},${valueToY(0)}Z`,
              };
            }),
          ),
    { equals: (previous, next) => haveSameItems(previous, next, isSameLineRun) },
  );

  const [hoveredBucketIndex, setHoveredBucketIndex] = createSignal<number>();
  const [zoomDrag, setZoomDrag] = createSignal<ZoomDrag>();

  const hoveredPoints = createMemo<HoveredPoint[]>(() => {
    const bucketIndex = hoveredBucketIndex();
    if (bucketIndex === undefined) return [];
    return shownSeries().flatMap((series) => {
      const value = series.values[bucketIndex];
      return value === null || value === undefined ? [] : [{ cssColor: series.cssColor, value }];
    });
  });

  const findBucketIndexAt = (x: number) => {
    const bucketCount = frame().bucketStartsMs.length;
    if (bucketCount === 0) return undefined;
    const bucketIndex = Math.floor(
      (xToTimeMs(x) - (frame().bucketStartsMs[0] ?? frame().startMs)) / frame().stepMs,
    );
    return Math.min(bucketCount - 1, Math.max(0, bucketIndex));
  };
  const pointerX = (e: PointerEvent) => e.clientX - plotElement.getBoundingClientRect().left;
  const clampToPlot = (x: number) =>
    Math.min(PLOT_LEFT_PX + plotWidth(), Math.max(PLOT_LEFT_PX, x));

  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    plotElement.setPointerCapture(e.pointerId);
    setZoomDrag({ fromX: clampToPlot(pointerX(e)), toX: clampToPlot(pointerX(e)) });
  };
  const onPointerMove = (e: PointerEvent) => {
    const x = pointerX(e);
    setHoveredBucketIndex(x < PLOT_LEFT_PX - 4 ? undefined : findBucketIndexAt(x));
    const drag = zoomDrag();
    if (drag) setZoomDrag({ ...drag, toX: clampToPlot(x) });
  };
  const onPointerUp = () => {
    const drag = zoomDrag();
    setZoomDrag(undefined);
    flush();
    if (!drag || Math.abs(drag.toX - drag.fromX) < MIN_ZOOM_DRAG_PX) return;
    props.onZoom(
      xToTimeMs(Math.min(drag.fromX, drag.toX)),
      xToTimeMs(Math.max(drag.fromX, drag.toX)),
    );
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const bucketCount = frame().bucketStartsMs.length;
    if (bucketCount === 0) return;
    switch (e.key) {
      case "ArrowLeft":
        setHoveredBucketIndex((bucketIndex) => Math.max(0, (bucketIndex ?? bucketCount) - 1));
        break;
      case "ArrowRight":
        setHoveredBucketIndex((bucketIndex) => Math.min(bucketCount - 1, (bucketIndex ?? -1) + 1));
        break;
      case "Home":
        setHoveredBucketIndex(0);
        break;
      case "End":
        setHoveredBucketIndex(bucketCount - 1);
        break;
      case "Escape":
        if (hoveredBucketIndex() === undefined) return;
        setHoveredBucketIndex(undefined);
        break;
      default:
        return;
    }
    e.preventDefault();
  };

  const sumBucket = (bucketIndex: number) =>
    shownSeries().reduce((sum, series) => sum + (series.values[bucketIndex] ?? 0), 0);

  return (
    <div class={["relative min-w-0", { "flex min-h-0 flex-1 flex-col": props.height === "fill" }]}>
      <div
        ref={plotElement}
        class={[
          "relative cursor-crosshair touch-none rounded-md outline-offset-2 select-none",
          { "min-h-0 flex-1": props.height === "fill" },
        ]}
        style={props.height === "fill" ? {} : { height: `${canvasHeight()}px` }}
        tabindex={0}
        role="group"
        aria-label={`${props.label}. The arrow keys read the values of each step.`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => {
          if (!zoomDrag()) setHoveredBucketIndex(undefined);
        }}
        onKeyDown={onKeyDown}
        onBlur={() => setHoveredBucketIndex(undefined)}
      >
        <Show when={chartWidth() > 0}>
          <svg
            width={chartWidth()}
            height={canvasHeight()}
            class={["block overflow-visible transition-opacity", { "opacity-50": props.loading }]}
            aria-hidden="true"
          >
            <For each={yTicks()} keyed={false}>
              {(tick) => (
                <>
                  <line
                    x1={PLOT_LEFT_PX}
                    x2={PLOT_LEFT_PX + plotWidth()}
                    y1={Math.round(valueToY(tick())) + 0.5}
                    y2={Math.round(valueToY(tick())) + 0.5}
                    stroke={tick() === 0 ? "var(--color-line-focus)" : "var(--color-line)"}
                    stroke-width="1"
                  />
                  <text
                    x={PLOT_LEFT_PX - 8}
                    y={valueToY(tick())}
                    dy="0.32em"
                    text-anchor="end"
                    class="fill-muted text-[11px] tabular-nums"
                  >
                    {formatValue(tick(), props.unit)}
                  </text>
                </>
              )}
            </For>
            <For each={xTicks()} keyed={false}>
              {(tick) => (
                <text
                  x={timeToX(tick())}
                  y={PLOT_TOP_PX + plotHeight() + 15}
                  text-anchor="middle"
                  class="fill-muted text-[11px] tabular-nums"
                >
                  {formatTick(tick())}
                </text>
              )}
            </For>

            <Show when={hoveredBucketIndex() !== undefined && props.kind === "bar"}>
              <rect
                x={bucketCenterX(hoveredBucketIndex() ?? 0) - barWidth() / 2 - 3}
                y={PLOT_TOP_PX}
                width={barWidth() + 6}
                height={plotHeight()}
                rx="3"
                fill="var(--color-hover)"
              />
            </Show>

            <For each={barPaths()} keyed={false}>
              {(bar) => <path d={bar().path} fill={bar().cssColor} />}
            </For>
            <Show when={props.kind === "area"}>
              <For each={lineRuns()} keyed={false}>
                {(run) => <path d={run().area} fill={run().cssColor} fill-opacity="0.1" />}
              </For>
            </Show>
            <For each={lineRuns()} keyed={false}>
              {(run) => (
                <path
                  d={run().line}
                  fill="none"
                  stroke={run().cssColor}
                  stroke-width="2"
                  stroke-linejoin="round"
                  stroke-linecap="round"
                />
              )}
            </For>

            <Show when={hoveredBucketIndex() !== undefined && props.kind !== "bar"}>
              <line
                x1={Math.round(bucketCenterX(hoveredBucketIndex() ?? 0)) + 0.5}
                x2={Math.round(bucketCenterX(hoveredBucketIndex() ?? 0)) + 0.5}
                y1={PLOT_TOP_PX}
                y2={PLOT_TOP_PX + plotHeight()}
                stroke="var(--color-muted)"
                stroke-width="1"
              />
              <For each={hoveredPoints()} keyed={false}>
                {(point) => (
                  <circle
                    cx={bucketCenterX(hoveredBucketIndex() ?? 0)}
                    cy={valueToY(point().value)}
                    r="4"
                    fill={point().cssColor}
                    stroke="var(--viz-surface, var(--color-surface))"
                    stroke-width="2"
                  />
                )}
              </For>
            </Show>

            <Show when={zoomDrag()}>
              {(drag) => (
                <rect
                  x={Math.min(drag().fromX, drag().toX)}
                  y={PLOT_TOP_PX}
                  width={Math.abs(drag().toX - drag().fromX)}
                  height={plotHeight()}
                  fill="var(--color-accent)"
                  fill-opacity="0.12"
                />
              )}
            </Show>
          </svg>
        </Show>

        <Show when={hasNoValues()}>
          <div
            class="absolute inset-x-0 flex items-center justify-center text-sm text-muted"
            style={{
              top: `${PLOT_TOP_PX}px`,
              height: `${plotHeight()}px`,
              left: `${PLOT_LEFT_PX}px`,
            }}
          >
            {props.emptyMessage ?? "No data in this range"}
          </div>
        </Show>

        <Show when={hoveredBucketIndex() !== undefined && !zoomDrag() && !hasNoValues()}>
          <ChartPanelTooltip
            crosshairX={bucketCenterX(hoveredBucketIndex() ?? 0)}
            chartWidth={chartWidth()}
            bucketStartMs={frame().bucketStartsMs[hoveredBucketIndex() ?? 0] ?? 0}
            stepMs={frame().stepMs}
            rows={shownSeries().map((series) => ({
              label: series.label,
              cssColor: series.cssColor,
              value: series.values[hoveredBucketIndex() ?? 0] ?? null,
            }))}
            total={
              props.kind === "bar" && shownSeries().length > 1
                ? sumBucket(hoveredBucketIndex() ?? 0)
                : undefined
            }
            unit={props.unit}
          />
        </Show>
      </div>

      {/* A table ignores a height below its content, so the box that hides
          it is a div. */}
      <div class="sr-only">
        <table>
          <caption>{props.label}</caption>
          <thead>
            <tr>
              <th scope="col">Time</th>
              <For each={coloredSeries()}>{(series) => <th scope="col">{series.label}</th>}</For>
            </tr>
          </thead>
          <tbody>
            <For each={frame().bucketStartsMs}>
              {(bucketStartMs, bucketIndex) => (
                <tr>
                  <th scope="row">{formatInstant(bucketStartMs)}</th>
                  <For each={coloredSeries()}>
                    {(series) => <td>{formatValue(series.values[bucketIndex()], props.unit)}</td>}
                  </For>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>
    </div>
  );
}

function haveSameItems<T>(
  previous: ReadonlyArray<T>,
  next: ReadonlyArray<T>,
  isSameItem: (previousItem: T, nextItem: T) => boolean = Object.is,
): boolean {
  return (
    previous.length === next.length &&
    previous.every((item, index) => {
      const nextItem = next[index];
      return nextItem !== undefined && isSameItem(item, nextItem);
    })
  );
}

function isSameBarPath(previous: BarPath, next: BarPath): boolean {
  return previous.cssColor === next.cssColor && previous.path === next.path;
}

function isSameLineRun(previous: LineRun, next: LineRun): boolean {
  return (
    previous.cssColor === next.cssColor &&
    previous.line === next.line &&
    previous.area === next.area
  );
}

function toBarPath(x: number, y: number, width: number, height: number, roundTop: boolean): string {
  const radius = roundTop ? Math.min(BAR_CORNER_RADIUS_PX, width / 2, height) : 0;
  return `M${x},${y + height}V${y + radius}Q${x},${y} ${x + radius},${y}H${x + width - radius}Q${x + width},${y} ${x + width},${y + radius}V${y + height}Z`;
}

function toLoneValueDash(x: number, y: number): string {
  return `M${x - LONE_VALUE_DASH_WIDTH_PX / 2},${y}h${LONE_VALUE_DASH_WIDTH_PX}`;
}

interface BucketValue {
  readonly bucketIndex: number;
  readonly value: number;
}

function splitIntoRuns(values: ReadonlyArray<number | null>): BucketValue[][] {
  const runs: BucketValue[][] = [];
  let run: BucketValue[] = [];
  values.forEach((value, bucketIndex) => {
    if (value === null) {
      if (run.length > 0) runs.push(run);
      run = [];
    } else {
      run.push({ bucketIndex, value });
    }
  });
  if (run.length > 0) runs.push(run);
  return runs;
}
