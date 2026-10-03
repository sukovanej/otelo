import type { SpanSort } from "@otelo/api";
import type { TableSorting, TableSortOrder } from "@otelo/viz";

export const SPAN_SORTS = [
  "newest",
  "oldest",
  "longest",
  "shortest",
] as const satisfies ReadonlyArray<SpanSort>;

export const SPAN_SORT_DESCRIPTIONS: Record<SpanSort, string> = {
  newest: "newest first",
  oldest: "oldest first",
  longest: "longest first",
  shortest: "shortest first",
};

const TABLE_SORT_ORDERS: Record<SpanSort, TableSortOrder> = {
  newest: { columnId: "time", descending: true },
  oldest: { columnId: "time", descending: false },
  longest: { columnId: "duration", descending: true },
  shortest: { columnId: "duration", descending: false },
};

const SORTED_COLUMN_IDS = ["time", "duration"];

export interface SpanSorting {
  readonly sort: SpanSort;
  readonly onSort: (sort: SpanSort) => void;
}

export function toTableSorting(sorting: SpanSorting): TableSorting {
  return {
    kind: "source",
    order: TABLE_SORT_ORDERS[sorting.sort],
    columnIds: SORTED_COLUMN_IDS,
    onSort: (order) => sorting.onSort(toSpanSort(order)),
  };
}

function toSpanSort(order: TableSortOrder): SpanSort {
  return (
    SPAN_SORTS.find(
      (sort) =>
        TABLE_SORT_ORDERS[sort].columnId === order.columnId &&
        TABLE_SORT_ORDERS[sort].descending === order.descending,
    ) ?? "newest"
  );
}
