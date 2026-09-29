// The colors a series can take, by name, so a dashboard can name them in
// JSON. Each is a color of the theme of `@otelo/ui`.

/** `series-1` to `series-8` tell series apart, in this order. `error`,
 * `warn`, and `success` mean a state and never tell series apart. `muted` is
 * the series that matters least, such as "other". `p50`, `p95`, and `p99` are
 * the percentiles of one measure. */
export type SeriesColor =
  | "series-1"
  | "series-2"
  | "series-3"
  | "series-4"
  | "series-5"
  | "series-6"
  | "series-7"
  | "series-8"
  | "accent"
  | "error"
  | "warn"
  | "success"
  | "muted"
  | "p50"
  | "p95"
  | "p99";

/** The colors that tell series apart, in the order they are given out. */
export const CATEGORICAL: readonly SeriesColor[] = [
  "series-1",
  "series-2",
  "series-3",
  "series-4",
  "series-5",
  "series-6",
  "series-7",
  "series-8",
];

/** The CSS color of `color`. */
export const cssColor = (color: SeriesColor) =>
  color === "muted" ? "var(--color-trace)" : `var(--color-${color})`;

/** The color of the series at `index` when it names none: the categorical
 * colors in order, and `muted` past them, since a ninth hue would look like
 * one of the eight. */
export const defaultColor = (index: number): SeriesColor => CATEGORICAL[index] ?? "muted";
