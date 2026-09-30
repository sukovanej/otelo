import type { JSX } from "solid-js";

import type { SortValue } from "./sort";
import type { Unit } from "./units";

const ALIGN_CLASSES: Record<ColumnAlign, string> = {
  start: "",
  end: "justify-end text-right",
};

const DEFAULT_TRACKS: Record<ColumnKind, string> = {
  number: "max-content",
  meter: "minmax(16ch,1.2fr)",
  text: "minmax(12ch,2fr)",
};

export interface Column<R> {
  readonly id: string;
  readonly label: string;
  /** What the column sorts by, and shows when it has no `cell`. */
  readonly value: (row: R) => SortValue;
  readonly unit?: Unit;
  readonly cell?: (row: R) => JSX.Element;
  readonly header?: () => JSX.Element;
  /** A bar before each number of its share of the largest in the column. */
  readonly meter?: boolean;
  readonly align?: ColumnAlign;
  readonly tone?: (row: R) => CellTone | undefined;
  readonly width?: string;
  readonly description?: string;
  /** A column sorts on a click on its header when the table sorts, unless
   * this is false. */
  readonly sortable?: boolean;
}

export type CellTone = "error" | "warn" | "muted";

type ColumnAlign = "start" | "end";

type ColumnKind = "number" | "meter" | "text";

export function resolveColumnAlign<R>(column: Column<R>): ColumnAlign {
  return column.align ?? (column.unit ? "end" : "start");
}

export function pickAlignClass<R>(column: Column<R>): string {
  return ALIGN_CLASSES[resolveColumnAlign(column)];
}

export function pickGridTrack<R>(column: Column<R>): string {
  return column.width ?? DEFAULT_TRACKS[pickColumnKind(column)];
}

function pickColumnKind<R>(column: Column<R>): ColumnKind {
  if (column.meter) return "meter";
  return column.unit ? "number" : "text";
}
