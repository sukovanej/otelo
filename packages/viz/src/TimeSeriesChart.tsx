import { createMemo, createSignal, For, Index, onCleanup, onMount, Show } from "solid-js";

import { cssColor, defaultColor, type SeriesColor } from "./color";
import Legend from "./Legend";
import { formatInstant, formatTick, niceTicks, timeTicks } from "./scale";
import { formatValue, type Unit } from "./units";
import Value from "./Value";

/** The buckets of time that the values of a chart fall in. */
export interface TimeFrame {
  /** The start of each bucket, in milliseconds since the epoch. */
  times: number[];
  /** The length of a bucket, in milliseconds. */
  step: number;
  /** The range the chart spans, in milliseconds since the epoch. */
  start: number;
  end: number;
}

/** One series: a value for each bucket of the frame, `null` where it has
 * none, which breaks a line. */
export interface TimeSeries {
  label: string;
  values: (number | null)[];
  /** The categorical color of its place in the list when missing. */
  color?: SeriesColor;
}

/** `bar` stacks the series in bars, the first at the bottom. `line` draws a
 * line for each, and `area` a line over a faint wash. */
export type ChartKind = "line" | "area" | "bar";

const AXIS_HEIGHT = 20;
const LEFT = 52;
const RIGHT = 4;
const TOP = 8;
/** The surface between two stacked segments or two bars. */
const GAP = 2;
const RADIUS = 3;
const MAX_BAR = 24;
/** How far the pointer moves before a press becomes a drag that zooms. */
const DRAG = 4;

/** A bar from `y` down to `y + h`, with its top corners rounded when it is
 * the top of its stack and wide enough. */
function barPath(x: number, y: number, w: number, h: number, round: boolean): string {
  const r = round ? Math.min(RADIUS, w / 2, h) : 0;
  return `M${x},${y + h}V${y + r}Q${x},${y} ${x + r},${y}H${x + w - r}Q${x + w},${y} ${x + w},${y + r}V${y + h}Z`;
}

/** The runs of values of a line that no missing value breaks, as pairs of a
 * bucket and its value. */
function runs(values: (number | null)[]): [number, number][][] {
  const all: [number, number][][] = [];
  let run: [number, number][] = [];
  values.forEach((value, i) => {
    if (value === null) {
      if (run.length > 0) all.push(run);
      run = [];
    } else {
      run.push([i, value]);
    }
  });
  if (run.length > 0) all.push(run);
  return all;
}

/**
 * Values over time, in buckets of one step: stacked bars, lines, or areas,
 * on one axis in one unit.
 *
 * A crosshair follows the pointer or the arrow keys, and a tip lists every
 * series at its bucket. A click on an entry of the legend shows that series
 * alone, and a second click shows them all again. Dragging across the plot
 * calls `onZoom` with the stretch. While `loading`, the chart keeps its
 * values, fainter. A table of the values stands in for the chart for screen
 * readers.
 */
export default function TimeSeriesChart(props: {
  frame: TimeFrame;
  series: TimeSeries[];
  kind: ChartKind;
  unit: Unit;
  /** The height of the plot, without the axis under it. 150 when missing. */
  height?: number;
  loading?: boolean;
  /** What the chart says when no series has a value. */
  empty?: string;
  onZoom?: (start: number, end: number) => void;
  label?: string;
  /** A legend over the plot when there are two series or more, unless this
   * is false, such as when `ChartPanel` shows it beside the title. */
  legend?: boolean;
  /** The series the legend shows alone, when the legend is outside the
   * chart and holds it with `onIsolate`. */
  isolated?: number;
  onIsolate?: (index: number | undefined) => void;
}) {
  let box!: HTMLDivElement;
  const [width, setWidth] = createSignal(0);
  onMount(() => {
    setWidth(box.clientWidth);
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) setWidth(entry.contentRect.width);
    });
    observer.observe(box);
    onCleanup(() => observer.disconnect());
  });

  const plotHeight = () => props.height ?? 150;
  const plotWidth = () => Math.max(40, width() - LEFT - RIGHT);
  const frame = createMemo(() => props.frame);
  const span = () => Math.max(1, frame().end - frame().start);
  const x = (ms: number) => LEFT + ((ms - frame().start) / span()) * plotWidth();
  const msAt = (px: number) => frame().start + ((px - LEFT) / plotWidth()) * span();
  const center = (i: number) => x((frame().times[i] ?? frame().start) + frame().step / 2);

  // The series the legend isolated, by its place in the list: the one of
  // `props.isolated` when a legend outside the chart holds it.
  const [own, setOwn] = createSignal<number>();
  const isolated = () => (props.onIsolate ? props.isolated : own());
  const setIsolated = (index: number | undefined) =>
    props.onIsolate ? props.onIsolate(index) : setOwn(index);
  const all = createMemo(() =>
    props.series.map((s, i) => ({ ...s, css: cssColor(s.color ?? defaultColor(i)), index: i })),
  );
  const shown = createMemo(() => {
    const only = isolated();
    return only === undefined || only >= all().length
      ? all()
      : all().filter((s) => s.index === only);
  });
  const empty = createMemo(() => all().every((s) => s.values.every((v) => v === null)));

  /** The top of each bucket: the sum of a stack, or the largest value. */
  const tops = createMemo(() =>
    frame().times.map((_, i) => {
      const values = shown().map((s) => s.values[i] ?? 0);
      return props.kind === "bar" ? values.reduce((a, b) => a + b, 0) : Math.max(0, ...values);
    }),
  );
  const ticks = createMemo(() => niceTicks(Math.max(0, ...tops())));
  const top = createMemo(() => ticks().at(-1) || 1);
  const y = (value: number) => TOP + plotHeight() - (value / top()) * plotHeight();
  const xTicks = createMemo(() =>
    timeTicks(frame().start, frame().end, Math.floor(plotWidth() / 96)),
  );

  const barWidth = createMemo(() => {
    const slot = (frame().step / span()) * plotWidth();
    return Math.max(1, Math.min(MAX_BAR, slot - Math.min(GAP, slot / 3)));
  });

  /** The segments of each bar, bottom first, with a gap between them. */
  const bars = createMemo(() =>
    props.kind !== "bar"
      ? []
      : frame().times.flatMap((_, i) => {
          const w = barWidth();
          const left = center(i) - w / 2;
          let base = 0;
          const stack = shown().flatMap((s) => {
            const value = s.values[i] ?? 0;
            const segment = { css: s.css, from: base, to: base + value };
            base += value;
            return value > 0 ? [segment] : [];
          });
          return stack.map((s, n) => {
            const bottom = y(s.from) - (n > 0 ? GAP / 2 : 0);
            const upper = y(s.to) + (n < stack.length - 1 ? GAP / 2 : 0);
            const h = Math.max(1, bottom - upper);
            return {
              css: s.css,
              d: barPath(left, bottom - h, w, h, n === stack.length - 1 && w >= 6),
            };
          });
        }),
  );

  const lines = createMemo(() =>
    props.kind === "bar"
      ? []
      : shown().map((s) => ({
          css: s.css,
          runs: runs(s.values).map((run) => {
            const points = run.map(([i, v]) => `${center(i)},${y(v)}`);
            const [first] = run[0] ?? [0];
            const [last] = run.at(-1) ?? [first];
            return {
              // A run of one value is a short dash across its bucket.
              line:
                run.length === 1
                  ? `M${center(first) - 3},${y(run[0]?.[1] ?? 0)}h6`
                  : `M${points.join("L")}`,
              area: `M${center(first)},${y(0)}L${points.join("L")}L${center(last)},${y(0)}Z`,
            };
          }),
        })),
  );

  const [hover, setHover] = createSignal<number>();
  const [drag, setDrag] = createSignal<{ from: number; to: number }>();

  const indexAt = (px: number) => {
    const count = frame().times.length;
    if (count === 0) return undefined;
    const i = Math.floor((msAt(px) - (frame().times[0] ?? frame().start)) / frame().step);
    return Math.min(count - 1, Math.max(0, i));
  };
  const pointerX = (e: PointerEvent) => e.clientX - box.getBoundingClientRect().left;
  const clampX = (px: number) => Math.min(LEFT + plotWidth(), Math.max(LEFT, px));

  const onPointerDown = (e: PointerEvent) => {
    if (!props.onZoom || e.button !== 0) return;
    box.setPointerCapture(e.pointerId);
    setDrag({ from: clampX(pointerX(e)), to: clampX(pointerX(e)) });
  };
  const onPointerMove = (e: PointerEvent) => {
    const px = pointerX(e);
    setHover(px < LEFT - 4 ? undefined : indexAt(px));
    const d = drag();
    if (d) setDrag({ ...d, to: clampX(px) });
  };
  const onPointerUp = () => {
    const d = drag();
    setDrag(undefined);
    if (!d || !props.onZoom || Math.abs(d.to - d.from) < DRAG) return;
    props.onZoom(msAt(Math.min(d.from, d.to)), msAt(Math.max(d.from, d.to)));
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const count = frame().times.length;
    if (count === 0) return;
    if (e.key === "ArrowLeft") setHover((i) => Math.max(0, (i ?? count) - 1));
    else if (e.key === "ArrowRight") setHover((i) => Math.min(count - 1, (i ?? -1) + 1));
    else if (e.key === "Home") setHover(0);
    else if (e.key === "End") setHover(count - 1);
    else if (e.key === "Escape") setHover(undefined);
    else return;
    e.preventDefault();
  };

  const total = (i: number) => shown().reduce((sum, s) => sum + (s.values[i] ?? 0), 0);

  return (
    <div class="relative min-w-0">
      <Show when={props.legend !== false && all().length > 1}>
        <div class="mb-2">
          <Legend
            series={props.series}
            kind={props.kind}
            isolated={isolated()}
            onIsolate={setIsolated}
          />
        </div>
      </Show>

      <div
        ref={box}
        class="relative touch-none rounded-md outline-offset-2 select-none"
        classList={{ "cursor-crosshair": !!props.onZoom }}
        style={{ height: `${TOP + plotHeight() + AXIS_HEIGHT}px` }}
        tabIndex={0}
        role="group"
        aria-label={`${props.label ?? "Chart"}. The arrow keys read the values of each step.`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => {
          if (!drag()) setHover(undefined);
        }}
        onKeyDown={onKeyDown}
        onBlur={() => setHover(undefined)}
      >
        <Show when={width() > 0}>
          <svg
            width={width()}
            height={TOP + plotHeight() + AXIS_HEIGHT}
            class="block overflow-visible transition-opacity"
            classList={{ "opacity-50": props.loading }}
            aria-hidden="true"
          >
            <Index each={ticks()}>
              {(tick) => (
                <>
                  <line
                    x1={LEFT}
                    x2={LEFT + plotWidth()}
                    y1={Math.round(y(tick())) + 0.5}
                    y2={Math.round(y(tick())) + 0.5}
                    stroke={tick() === 0 ? "var(--color-line-focus)" : "var(--color-line)"}
                    stroke-width="1"
                  />
                  <text
                    x={LEFT - 8}
                    y={y(tick())}
                    dy="0.32em"
                    text-anchor="end"
                    class="fill-muted text-[11px] tabular-nums"
                  >
                    {formatValue(tick(), props.unit)}
                  </text>
                </>
              )}
            </Index>
            <Index each={xTicks()}>
              {(tick) => (
                <text
                  x={x(tick())}
                  y={TOP + plotHeight() + 15}
                  text-anchor="middle"
                  class="fill-muted text-[11px] tabular-nums"
                >
                  {formatTick(tick())}
                </text>
              )}
            </Index>

            <Show when={hover() !== undefined && props.kind === "bar"}>
              <rect
                x={center(hover() ?? 0) - barWidth() / 2 - 3}
                y={TOP}
                width={barWidth() + 6}
                height={plotHeight()}
                rx="3"
                fill="var(--color-hover)"
              />
            </Show>

            <For each={bars()}>{(bar) => <path d={bar.d} fill={bar.css} />}</For>
            <For each={lines()}>
              {(line) => (
                <For each={line.runs}>
                  {(run) => (
                    <>
                      <Show when={props.kind === "area"}>
                        <path d={run.area} fill={line.css} fill-opacity="0.1" />
                      </Show>
                      <path
                        d={run.line}
                        fill="none"
                        stroke={line.css}
                        stroke-width="2"
                        stroke-linejoin="round"
                        stroke-linecap="round"
                      />
                    </>
                  )}
                </For>
              )}
            </For>

            <Show when={hover() !== undefined && props.kind !== "bar"}>
              <line
                x1={Math.round(center(hover() ?? 0)) + 0.5}
                x2={Math.round(center(hover() ?? 0)) + 0.5}
                y1={TOP}
                y2={TOP + plotHeight()}
                stroke="var(--color-muted)"
                stroke-width="1"
              />
              <For each={shown()}>
                {(s) => (
                  <Show
                    when={s.values[hover() ?? 0] !== null && s.values[hover() ?? 0] !== undefined}
                  >
                    <circle
                      cx={center(hover() ?? 0)}
                      cy={y(s.values[hover() ?? 0] ?? 0)}
                      r="4"
                      fill={s.css}
                      stroke="var(--viz-surface, var(--color-surface))"
                      stroke-width="2"
                    />
                  </Show>
                )}
              </For>
            </Show>

            <Show when={drag()}>
              {(d) => (
                <rect
                  x={Math.min(d().from, d().to)}
                  y={TOP}
                  width={Math.abs(d().to - d().from)}
                  height={plotHeight()}
                  fill="var(--color-accent)"
                  fill-opacity="0.12"
                />
              )}
            </Show>
          </svg>
        </Show>

        <Show when={empty()}>
          <div
            class="absolute inset-x-0 flex items-center justify-center text-sm text-muted"
            style={{ top: `${TOP}px`, height: `${plotHeight()}px`, left: `${LEFT}px` }}
          >
            {props.empty ?? "No data in this range"}
          </div>
        </Show>

        <Show when={hover() !== undefined && !drag() && !empty()}>
          <Tip
            at={center(hover() ?? 0)}
            width={width()}
            start={frame().times[hover() ?? 0] ?? 0}
            step={frame().step}
            rows={shown().map((s) => ({
              label: s.label,
              css: s.css,
              value: s.values[hover() ?? 0] ?? null,
            }))}
            total={props.kind === "bar" && shown().length > 1 ? total(hover() ?? 0) : undefined}
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
              <For each={all()}>{(s) => <th scope="col">{s.label}</th>}</For>
            </tr>
          </thead>
          <tbody>
            <For each={frame().times}>
              {(time, i) => (
                <tr>
                  <th scope="row">{formatInstant(time)}</th>
                  <For each={all()}>{(s) => <td>{formatValue(s.values[i()], props.unit)}</td>}</For>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>
    </div>
  );
}

/** The values of every shown series at one bucket, beside the crosshair and
 * on the side of it with more room. */
function Tip(props: {
  at: number;
  width: number;
  start: number;
  step: number;
  rows: { label: string; css: string; value: number | null }[];
  total: number | undefined;
  unit: Unit;
}) {
  return (
    <div
      class="pointer-events-none absolute top-1 z-10 min-w-40 rounded-md border border-line bg-surface px-2.5 py-2 text-xs shadow-popup"
      style={
        props.at > props.width / 2
          ? { right: `${props.width - props.at + 12}px` }
          : { left: `${props.at + 12}px` }
      }
    >
      <div class="mb-1.5 whitespace-nowrap text-muted tabular-nums">
        {formatInstant(props.start)} – {formatInstant(props.start + props.step)}
      </div>
      <table class="border-collapse">
        <tbody>
          <For each={props.rows}>
            {(row) => (
              <tr>
                <td class="py-px pr-2">
                  <span class="block h-0.5 w-3 rounded-full" style={{ background: row.css }} />
                </td>
                <td class="py-px pr-3 text-right">
                  <Value value={row.value} unit={props.unit} />
                </td>
                <td class="py-px whitespace-nowrap text-muted">{row.label}</td>
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
