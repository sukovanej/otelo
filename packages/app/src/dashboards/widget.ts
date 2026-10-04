import type {
  AttributeKeys,
  ChartKind,
  GroupedQuery,
  MetricAggregation,
  RankOrder,
  SpanMeasure,
  Widget,
  WidgetDisplay,
  WidgetQuery,
} from "@otelo/api";
import {
  AreaChartIcon,
  BarChartIcon,
  BarListIcon,
  LineChartIcon,
  LineIcon,
  LogsIcon,
  MetricsIcon,
  NoteIcon,
  NumberIcon,
  SpanIcon,
} from "@otelo/icons";
import type { SelectOption, TabOption } from "@otelo/ui";

import { writeAttributeField, writeResourceField } from "../query";

export const GRID_ROW_HEIGHT_PX = 40;

export const GRID_GAP_PX = 12;

export const DISPLAY_KIND_OPTIONS: ReadonlyArray<TabOption<DisplayKind>> = [
  { value: "timeseries", label: "Time series", icon: LineChartIcon },
  { value: "value", label: "Value", icon: NumberIcon },
  { value: "toplist", label: "Top list", icon: BarListIcon },
  { value: "note", label: "Note", icon: NoteIcon },
];

export const CHART_KIND_OPTIONS: ReadonlyArray<TabOption<ChartKind>> = [
  { value: "line", label: "Line", icon: LineIcon },
  { value: "area", label: "Area", icon: AreaChartIcon },
  { value: "bar", label: "Bars", icon: BarChartIcon },
];

export const SIGNAL_OPTIONS: ReadonlyArray<TabOption<QuerySignal>> = [
  { value: "spans", label: "Spans", icon: SpanIcon },
  { value: "logs", label: "Logs", icon: LogsIcon },
  { value: "metrics", label: "Metrics", icon: MetricsIcon },
];

export const SPAN_MEASURE_OPTIONS: ReadonlyArray<SelectOption<SpanMeasure>> = [
  { value: "count", label: "Count" },
  { value: "rate", label: "Rate" },
  { value: "errors", label: "Errors" },
  { value: "error_rate", label: "Error rate" },
  { value: "p50", label: "P50" },
  { value: "p95", label: "P95" },
  { value: "p99", label: "P99" },
];

export const METRIC_AGGREGATION_OPTIONS: ReadonlyArray<SelectOption<MetricAggregation>> = [
  { value: "avg", label: "Average" },
  { value: "min", label: "Minimum" },
  { value: "max", label: "Maximum" },
  { value: "last", label: "Last" },
  { value: "rate", label: "Rate" },
  { value: "p50", label: "P50" },
  { value: "p90", label: "P90" },
  { value: "p99", label: "P99" },
];

export const TOPLIST_LIMIT_OPTIONS: ReadonlyArray<SelectOption<string>> = [
  { value: "5", label: "5" },
  { value: "10", label: "10" },
  { value: "25", label: "25" },
  { value: "50", label: "50" },
];

export const TOPLIST_ORDER_OPTIONS: ReadonlyArray<SelectOption<RankOrder>> = [
  { value: "highest", label: "Highest first" },
  { value: "lowest", label: "Lowest first" },
];

export const MAX_QUERIES_PER_TIMESERIES = 8;

const BUILTIN_GROUPING_FIELDS: Record<QuerySignal, ReadonlyArray<string>> = {
  spans: ["service", "name"],
  logs: ["service", "level"],
  metrics: ["service"],
};

const DEFAULT_QUERY: WidgetQuery = { signal: "spans", filter: "", measure: "count" };

export type QuerySignal = WidgetQuery["signal"];

type DisplayKind = WidgetDisplay["kind"];

export function createDefaultWidget(): Widget {
  return {
    title: "",
    layout: { column: 0, row: 0, width: 6, height: 6 },
    display: { kind: "timeseries", chart: "line", queries: [{ query: DEFAULT_QUERY, by: [] }] },
  };
}

export function measureRowsHeightPx(rows: number): number {
  return rows * GRID_ROW_HEIGHT_PX + (rows - 1) * GRID_GAP_PX;
}

export function describeWidgetTitle(widget: Widget): string {
  if (widget.title.trim() !== "") return widget.title;
  const [firstQuery] = listWidgetQueries(widget.display);
  return firstQuery ? describeWidgetQuery(firstQuery.query) : "Note";
}

export function describeWidgetQuery(query: WidgetQuery): string {
  const filter = query.filter.trim() === "" ? "" : ` where ${query.filter.trim()}`;
  if (query.signal === "spans") {
    return `${findOptionLabel(SPAN_MEASURE_OPTIONS, query.measure)} of spans${filter}`;
  }
  if (query.signal === "logs") return `Logs${filter}`;
  const aggregation = findOptionLabel(METRIC_AGGREGATION_OPTIONS, query.aggregation);
  return `${aggregation} of ${query.name || "a metric"}${filter}`;
}

export function listWidgetQueries(display: WidgetDisplay): ReadonlyArray<GroupedQuery> {
  if (display.kind === "timeseries") return display.queries;
  if (display.kind === "value") return [{ query: display.query, by: [] }];
  if (display.kind === "toplist") return [display.query];
  return [];
}

export function changeDisplayKind(display: WidgetDisplay, kind: DisplayKind): WidgetDisplay {
  if (display.kind === kind) return display;
  const firstQuery = listWidgetQueries(display)[0] ?? { query: DEFAULT_QUERY, by: [] };
  const chart = display.kind === "timeseries" ? display.chart : "line";
  const displays: Record<DisplayKind, () => WidgetDisplay> = {
    timeseries: () => ({ kind: "timeseries", chart, queries: [firstQuery] }),
    value: () => ({ kind: "value", query: firstQuery.query }),
    toplist: () => ({
      kind: "toplist",
      query: {
        query: firstQuery.query,
        by: firstQuery.by.length > 0 ? firstQuery.by : ["service"],
      },
      limit: 10,
      order: "highest",
    }),
    note: () => ({ kind: "note", text: "" }),
  };
  return displays[kind]();
}

export function changeQuerySignal(query: WidgetQuery, signal: QuerySignal): WidgetQuery {
  if (query.signal === signal) return query;
  const queries: Record<QuerySignal, WidgetQuery> = {
    spans: { signal: "spans", filter: "", measure: "count" },
    logs: { signal: "logs", filter: "" },
    metrics: { signal: "metrics", name: "", filter: "", aggregation: "avg" },
  };
  return queries[signal];
}

export function listCatalogGroupingOptions(
  signal: QuerySignal,
  keys: AttributeKeys | undefined,
  chosen: ReadonlyArray<string>,
): SelectOption<string>[] {
  const builtinFields = BUILTIN_GROUPING_FIELDS[signal];
  const attributeFields = (keys?.record ?? [])
    .map((attribute) => writeAttributeField(attribute.key))
    .filter((field) => !field.startsWith("`"));
  const resourceFields = (keys?.resource ?? []).flatMap(
    (attribute) => writeResourceField(attribute.key) ?? [],
  );
  const knownFields = new Set([...builtinFields, ...attributeFields, ...resourceFields]);
  const unknownFields = chosen.filter((field) => !knownFields.has(field));
  return [
    ...builtinFields.map(toFieldOption("Fields")),
    ...[...attributeFields, ...unknownFields.filter((field) => !field.startsWith("resource."))].map(
      toFieldOption("Attributes"),
    ),
    ...[...resourceFields, ...unknownFields.filter((field) => field.startsWith("resource."))].map(
      toFieldOption("Resource"),
    ),
  ];
}

function toFieldOption(section: string): (field: string) => SelectOption<string> {
  return (field) => ({ value: field, label: field, section });
}

function findOptionLabel<T extends string>(
  options: ReadonlyArray<SelectOption<T>>,
  value: T,
): string {
  return options.find((option) => option.value === value)?.label ?? value;
}
