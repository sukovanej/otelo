import {
  getLogCounts,
  getMetricSeries,
  getSpanGroups,
  type GroupedQuery,
  type RankOrder,
  type SpanGroupRank,
  type SpanMeasure,
  type WidgetDisplay,
  type WidgetQuery,
} from "@otelo/api";

import type { RangeBounds } from "../services/range";
import {
  combineMeasuredQueries,
  type FoldedGroup,
  type MeasuredChart,
  type MeasuredQuery,
  measureLogCounts,
  measureMetricSeries,
  measureSpanGroups,
} from "./measure";
import { describeWidgetQuery } from "./widget";

const TIMESERIES_GROUP_LIMIT = 10;

const SPAN_MEASURE_RANKS: Record<SpanMeasure, SpanGroupRank> = {
  count: "count",
  rate: "count",
  errors: "errors",
  error_rate: "error_rate",
  p50: "p50",
  p95: "p95",
  p99: "p99",
};

const MAX_COMBINED_SERIES = 200;

const TIMESERIES_GROUPS: GroupRequest = {
  limit: TIMESERIES_GROUP_LIMIT,
  order: "highest",
  foldedGroup: "kept",
};

export type WidgetData = ChartData | MeasuredData | NoData;

// What a widget fetches: its display without what only changes the drawing, so
// a switch from lines to bars reads nothing again.
export type FetchedDisplay = FetchedTimeseries | FetchedValue | FetchedToplist;

interface ChartData {
  readonly kind: "chart";
  readonly chart: MeasuredChart;
}

interface MeasuredData {
  readonly kind: "measured";
  readonly measured: MeasuredQuery;
}

interface NoData {
  readonly kind: "none";
  readonly reason: string;
}

interface FetchedTimeseries {
  readonly kind: "timeseries";
  readonly queries: ReadonlyArray<GroupedQuery>;
}

interface FetchedValue {
  readonly kind: "value";
  readonly query: WidgetQuery;
}

interface FetchedToplist {
  readonly kind: "toplist";
  readonly query: GroupedQuery;
  readonly limit: number;
  readonly order: RankOrder;
}

// The groups a query asks the daemon for, which ranks them and keeps the limit.
interface GroupRequest {
  readonly limit: number;
  readonly order: RankOrder;
  readonly foldedGroup: FoldedGroup;
}

export function toFetchedDisplay(display: WidgetDisplay): FetchedDisplay | undefined {
  if (display.kind === "timeseries") return { kind: "timeseries", queries: display.queries };
  if (display.kind === "toplist") {
    return {
      kind: "toplist",
      query: display.query,
      limit: display.limit,
      order: display.order ?? "highest",
    };
  }
  return display.kind === "value" ? display : undefined;
}

export async function fetchWidgetData(
  display: FetchedDisplay,
  bounds: RangeBounds,
  signal: AbortSignal,
): Promise<WidgetData> {
  if (hasUnnamedMetric(display)) {
    return { kind: "none", reason: "Pick a metric to see its numbers." };
  }
  if (display.kind === "timeseries") {
    const measured = await Promise.all(
      display.queries.map((query) => fetchMeasuredQuery(query, TIMESERIES_GROUPS, bounds, signal)),
    );
    const chart = combineMeasuredQueries(
      measured.filter((query): query is MeasuredQuery => query !== undefined),
    );
    return chart ? { kind: "chart", chart } : { kind: "none", reason: "No series match." };
  }
  const grouped: GroupedQuery =
    display.kind === "value" ? { query: display.query, by: [] } : display.query;
  const request: GroupRequest =
    display.kind === "toplist"
      ? { limit: display.limit, order: display.order, foldedGroup: "dropped" }
      : { limit: 1, order: "highest", foldedGroup: "dropped" };
  const measured = await fetchMeasuredQuery(grouped, request, bounds, signal);
  return measured ? { kind: "measured", measured } : { kind: "none", reason: "No series match." };
}

async function fetchMeasuredQuery(
  grouped: GroupedQuery,
  request: GroupRequest,
  bounds: RangeBounds,
  signal: AbortSignal,
): Promise<MeasuredQuery | undefined> {
  const { query, by } = grouped;
  const label = describeWidgetQuery(query);
  const isGrouped = by.length > 0;
  if (query.signal === "spans") {
    const answer = await getSpanGroups(
      {
        ...bounds,
        q: query.filter,
        by: by.join(","),
        rank: SPAN_MEASURE_RANKS[query.measure],
        order: request.order,
        limit: request.limit,
        group_buckets: isGrouped,
      },
      signal,
    );
    return measureSpanGroups(answer, query.measure, by, label);
  }
  if (query.signal === "logs") {
    const answer = await getLogCounts(
      { ...bounds, q: query.filter, by: by.join(","), order: request.order, limit: request.limit },
      signal,
    );
    return measureLogCounts(answer, by, label);
  }
  const answer = await getMetricSeries(
    query.name.trim(),
    isGrouped
      ? {
          ...bounds,
          q: query.filter,
          by: by.join(","),
          top: request.limit,
          order: request.order,
          limit: request.limit + 1,
        }
      : { ...bounds, q: query.filter, limit: MAX_COMBINED_SERIES },
    signal,
  );
  return measureMetricSeries(answer, query.aggregation, by, label, request.foldedGroup);
}

function hasUnnamedMetric(display: FetchedDisplay): boolean {
  const queries =
    display.kind === "timeseries"
      ? display.queries.map((grouped) => grouped.query)
      : [display.kind === "value" ? display.query : display.query.query];
  return queries.some((query) => query.signal === "metrics" && query.name.trim() === "");
}
