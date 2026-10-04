// Every prop but the callbacks is plain data, with units and colors by name,
// so a dashboard can write them in JSON. An app adds `@source` for this
// package's `src`.

export { default as ChartPanel } from "./ChartPanel";
export type { SeriesColor } from "./color";
export type { Column } from "./column";
export { default as Panel } from "./Panel";
export type { ChartKind, TimeFrame, TimeSeries } from "./series";
export { default as Sparkline } from "./Sparkline";
export { default as Stat } from "./Stat";
export { default as Table, type TableSorting, type TableSortOrder } from "./Table";
export { formatValue, type Unit } from "./units";
export { default as Value } from "./Value";
