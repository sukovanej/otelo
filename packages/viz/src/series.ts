import type { SeriesColor } from "./color";

export type ChartKind = "line" | "area" | "bar";

export interface TimeFrame {
  /** The start of each bucket, in milliseconds since the epoch. */
  readonly times: ReadonlyArray<number>;
  /** The length of a bucket, in milliseconds. */
  readonly step: number;
  /** The range the chart spans, in milliseconds since the epoch. */
  readonly start: number;
  readonly end: number;
}

export interface TimeSeries {
  readonly label: string;
  readonly values: ReadonlyArray<number | null>;
  readonly color?: SeriesColor;
}
