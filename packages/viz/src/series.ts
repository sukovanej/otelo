import type { SeriesColor } from "./color";

export type ChartKind = "line" | "area" | "bar";

export interface TimeFrame {
  readonly bucketStartsMs: ReadonlyArray<number>;
  readonly stepMs: number;
  readonly startMs: number;
  readonly endMs: number;
}

export interface TimeSeries {
  readonly label: string;
  readonly values: ReadonlyArray<number | null>;
  readonly color?: SeriesColor;
}
