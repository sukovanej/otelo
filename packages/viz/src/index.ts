// The parts that show data in the otelo UI: charts of values over time,
// tables, and single numbers, each in a unit and in the colors of the theme
// of `@otelo/ui`, framed by a panel. Pages build on them, and so will
// dashboards: every prop but the callbacks is plain data, and units and
// colors go by name. An app adds `@source` for this package's `src`.

export { default as ChartPanel } from "./ChartPanel";
export { CATEGORICAL, cssColor, defaultColor, type SeriesColor } from "./color";
export { default as Legend } from "./Legend";
export { default as Meter } from "./Meter";
export { default as Panel } from "./Panel";
export { formatInstant, formatTick, niceTicks, timeTicks } from "./scale";
export { sortRows, type SortValue } from "./sort";
export { default as Sparkline } from "./Sparkline";
export { default as Stat, type StatTone } from "./Stat";
export { type Column, type SortOrder, default as Table, type Tone, type TreeLevel } from "./Table";
export {
  type ChartKind,
  type TimeFrame,
  type TimeSeries,
  default as TimeSeriesChart,
} from "./TimeSeriesChart";
export {
  durationParts,
  formatDuration,
  formatValue,
  type Unit,
  type ValuePart,
  valueParts,
} from "./units";
export { default as Value } from "./Value";
