import type { JSX } from "@solidjs/web";

import type { SortValue } from "./sort";
import type { Unit } from "./units";

const ALIGN_CLASSES: Record<ColumnAlign, string> = {
  start: "",
  end: "justify-end text-right",
};

const DEFAULT_ALIGNS: Record<ColumnKind, ColumnAlign> = {
  number: "end",
  meter: "end",
  text: "start",
  cell: "start",
};

const DEFAULT_TRACKS: Record<SizedColumnKind, string> = {
  number: "max-content",
  text: "minmax(12ch,2fr)",
  cell: "minmax(12ch,2fr)",
};

const DEFAULT_BAR_TRACK = "minmax(8ch,1.2fr)";

export type CellTone = "error" | "warn" | "muted";

export type Column<R> = NumberColumn<R> | MeterColumn<R> | TextColumn<R> | CellColumn<R>;

interface ColumnBase<R> {
  readonly id: string;
  readonly label: string;
  readonly description?: string;
  readonly tone?: (row: R) => CellTone | undefined;
}

interface SizedColumnBase<R> extends ColumnBase<R> {
  readonly width?: string;
}

interface NumberColumn<R> extends SizedColumnBase<R> {
  readonly kind: "number";
  readonly unit: Unit;
  readonly value: (row: R) => number | null;
}

interface MeterColumn<R> extends ColumnBase<R> {
  readonly kind: "meter";
  readonly barWidth?: string;
  readonly unit: Unit;
  readonly value: (row: R) => number | null;
}

interface TextColumn<R> extends SizedColumnBase<R> {
  readonly kind: "text";
  readonly value: (row: R) => string | null;
}

interface CellColumn<R> extends SizedColumnBase<R> {
  readonly kind: "cell";
  readonly cell: (row: R) => JSX.Element;
  readonly sortBy?: (row: R) => SortValue;
  readonly header?: () => JSX.Element;
  readonly align?: ColumnAlign;
}

type ColumnAlign = "start" | "end";

type ColumnKind = Column<unknown>["kind"];

type SizedColumnKind = Exclude<ColumnKind, "meter">;

export function resolveColumnAlign<R>(column: Column<R>): ColumnAlign {
  if (column.kind === "cell" && column.align) return column.align;
  return DEFAULT_ALIGNS[column.kind];
}

export function pickAlignClass<R>(column: Column<R>): string {
  return ALIGN_CLASSES[resolveColumnAlign(column)];
}

export function pickGridTracks<R>(column: Column<R>): string {
  if (column.kind === "meter") return `${column.barWidth ?? DEFAULT_BAR_TRACK} max-content`;
  return column.width ?? DEFAULT_TRACKS[column.kind];
}

export function pickSortValueReader<R>(column: Column<R>): ((row: R) => SortValue) | undefined {
  return column.kind === "cell" ? column.sortBy : column.value;
}
