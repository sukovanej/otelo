import { createMemo, createSignal, For, type JSX, Show } from "solid-js";

import { type Column, pickAlignClass, pickGridTrack, resolveColumnAlign } from "../column";
import { sortRows } from "../sort";
import TableRow, { type RowTone, type TreeLevel } from "./table-row";

interface SortOrder {
  readonly column: string;
  readonly descending: boolean;
}

interface TableProps<R> {
  readonly label: string;
  readonly rows: ReadonlyArray<R>;
  readonly columns: ReadonlyArray<Column<R>>;
  readonly sort?: SortOrder;
  readonly href?: (row: R) => string;
  readonly onRowClick?: (row: R) => void;
  readonly selected?: (row: R) => boolean;
  readonly tone?: (row: R) => RowTone | undefined;
  /** Whether the panel of `detail` shows under a row. */
  readonly expanded?: (row: R) => boolean;
  readonly detail?: (row: R) => JSX.Element;
  readonly level?: (row: R) => TreeLevel;
  readonly empty?: JSX.Element;
  readonly loading?: boolean;
}

export default function Table<R>(props: TableProps<R>) {
  const [sortOrder, setSortOrder] = createSignal<SortOrder | undefined>(props.sort);
  const isSortable = (column: Column<R>) => props.sort !== undefined && column.sortable !== false;
  const sortedRows = createMemo(() => {
    const order = sortOrder();
    const sortColumn = props.columns.find((column) => column.id === order?.column);
    return order && sortColumn
      ? sortRows(props.rows, sortColumn.value, order.descending)
      : props.rows;
  });
  const largestMeterValues = createMemo(() => {
    const largest = new Map<string, number>();
    for (const column of props.columns) {
      if (!column.meter) continue;
      const values = props.rows
        .map((row) => column.value(row))
        .filter((value) => typeof value === "number");
      largest.set(column.id, Math.max(0, ...values));
    }
    return largest;
  });
  const sortByColumn = (columnId: string) =>
    setSortOrder((order) =>
      order?.column === columnId
        ? { column: columnId, descending: !order.descending }
        : { column: columnId, descending: true },
    );

  return (
    <div
      role={props.level ? "treegrid" : "table"}
      aria-label={props.label}
      aria-busy={props.loading}
      class="grid gap-x-3 font-mono text-sm transition-opacity"
      classList={{ "opacity-60": props.loading }}
      style={{ "grid-template-columns": props.columns.map(pickGridTrack).join(" ") }}
    >
      <div
        role="row"
        class="sticky top-0 z-10 col-span-full grid grid-cols-subgrid items-center border-b border-line bg-(--viz-surface,var(--color-surface)) px-3 py-2 font-sans text-2xs font-semibold tracking-[0.04em] text-muted uppercase"
      >
        <For each={props.columns}>
          {(column) => {
            const isSorted = () => sortOrder()?.column === column.id;
            return (
              <div
                role="columnheader"
                aria-sort={
                  isSorted() ? (sortOrder()?.descending ? "descending" : "ascending") : undefined
                }
                aria-label={column.header ? column.label : undefined}
                class={`flex min-w-0 ${pickAlignClass(column)}`}
                title={column.description}
              >
                <Show
                  when={isSortable(column)}
                  fallback={
                    column.header ? column.header() : <span class="truncate">{column.label}</span>
                  }
                >
                  <button
                    type="button"
                    class="-mx-1 flex cursor-pointer items-baseline gap-1 rounded px-1 uppercase hover:text-ink"
                    classList={{
                      "text-ink": isSorted(),
                      "flex-row-reverse": resolveColumnAlign(column) === "end",
                    }}
                    onClick={() => sortByColumn(column.id)}
                  >
                    {column.label}
                    <span class={isSorted() ? "" : "invisible"} aria-hidden="true">
                      {sortOrder()?.descending ? "↓" : "↑"}
                    </span>
                  </button>
                </Show>
              </div>
            );
          }}
        </For>
      </div>

      <For
        each={sortedRows()}
        fallback={
          <div role="row" class="col-span-full px-3 py-8 text-center font-sans text-muted">
            <span role="cell">{props.empty ?? "Nothing to show."}</span>
          </div>
        }
      >
        {(row) => (
          <>
            <TableRow
              row={row}
              columns={props.columns}
              largestMeterValues={largestMeterValues()}
              href={props.href?.(row)}
              onRowClick={props.onRowClick}
              selected={props.selected?.(row)}
              tone={props.tone?.(row)}
              level={props.level?.(row)}
            />
            <Show when={props.detail && props.expanded?.(row)}>
              <div role="row" class="col-span-full border-b border-line">
                <div role="cell">{props.detail?.(row)}</div>
              </div>
            </Show>
          </>
        )}
      </For>
    </div>
  );
}
