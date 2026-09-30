const CATEGORICAL_COLORS: ReadonlyArray<CategoricalColor> = [
  "series-1",
  "series-2",
  "series-3",
  "series-4",
  "series-5",
  "series-6",
  "series-7",
  "series-8",
];

export type SeriesColor = CategoricalColor | StateColor | PercentileColor | "accent" | "muted";

type CategoricalColor =
  | "series-1"
  | "series-2"
  | "series-3"
  | "series-4"
  | "series-5"
  | "series-6"
  | "series-7"
  | "series-8";

type StateColor = "error" | "warn" | "success";

type PercentileColor = "p50" | "p95" | "p99";

// Each is a color of the theme of `@otelo/ui`.
export function toCssColor(color: SeriesColor): string {
  return color === "muted" ? "var(--color-trace)" : `var(--color-${color})`;
}

// A ninth hue would look like one of the eight.
export function pickDefaultColor(index: number): SeriesColor {
  return CATEGORICAL_COLORS[index] ?? "muted";
}
