import type { JSX } from "@solidjs/web";
import {
  type Accessor,
  createMemo,
  createProjection,
  createSignal,
  For,
  Show,
  untrack,
} from "solid-js";

import {
  type Column,
  pickAlignClass,
  pickGridTrack,
  pickSortValueReader,
  resolveColumnAlign,
} from "../column";
import { sortRows } from "../sort";
import TableRow, { type RowTone, type TreeLevel } from "./table-row";

interface SortOrder {
  readonly columnId: string;
  readonly descending: boolean;
}

interface TableProps<R> {
  readonly label: string;
  readonly rows: ReadonlyArray<R>;
  readonly rowKey: (row: R) => string;
  readonly columns: ReadonlyArray<Column<R>>;
  readonly initialSort?: SortOrder;
  readonly href?: (row: R) => string;
  readonly onRowClick?: (row: R) => void;
  readonly selectedKey?: Accessor<string | undefined>;
  readonly tone?: (row: R) => RowTone | undefined;
  readonly detail?: (row: R) => JSX.Element;
  readonly level?: (row: R) => TreeLevel;
  readonly emptyMessage?: JSX.Element;
  readonly loading?: boolean;
}

export default function Table<R>(props: TableProps<R>) {
  const [sortOrder, setSortOrder] = createSignal<SortOrder | undefined>(
    untrack(() => props.initialSort),
  );
  const isSortable = (column: Column<R>) =>
    props.initialSort !== undefined && pickSortValueReader(column) !== undefined;
  const sortedRows = createMemo(() => {
    const order = sortOrder();
    const sortColumn = props.columns.find((column) => column.id === order?.columnId);
    const readSortValue = sortColumn && pickSortValueReader(sortColumn);
    return order && readSortValue
      ? sortRows(props.rows, readSortValue, order.descending)
      : props.rows;
  });
  const largestMeterValues = createMemo(
    () => {
      const largest = new Map<string, number>();
      for (const column of props.columns) {
        if (column.kind !== "meter") continue;
        const values = props.rows.map((row) => column.value(row)).filter((value) => value !== null);
        largest.set(column.id, Math.max(0, ...values));
      }
      return largest;
    },
    { equals: haveSameEntries },
  );
  const selectedKeys = createProjection<Record<string, true>>(() => {
    const key = props.selectedKey?.();
    return key === undefined ? {} : { [key]: true };
  }, {});
  const sortByColumn = (columnId: string) =>
    setSortOrder((order) =>
      order?.columnId === columnId
        ? { columnId, descending: !order.descending }
        : { columnId, descending: true },
    );

  return (
    <div
      role={props.level ? "treegrid" : "table"}
      aria-label={props.label}
      aria-busy={props.loading ? "true" : undefined}
      class={["grid gap-x-3 font-mono text-sm transition-opacity", { "opacity-60": props.loading }]}
      style={{ "grid-template-columns": props.columns.map(pickGridTrack).join(" ") }}
    >
      <div
        role="row"
        class="sticky top-0 z-10 col-span-full grid grid-cols-subgrid items-center border-b border-line bg-(--viz-surface,var(--color-surface)) px-3 py-2 font-sans text-2xs font-semibold tracking-[0.04em] text-muted uppercase"
      >
        <For each={props.columns}>
          {(column) => {
            const isSorted = () => sortOrder()?.columnId === column.id;
            const drawHeader = column.kind === "cell" ? column.header : undefined;
            return (
              <div
                role="columnheader"
                aria-sort={
                  isSorted() ? (sortOrder()?.descending ? "descending" : "ascending") : undefined
                }
                aria-label={drawHeader ? column.label : undefined}
                class={`flex min-w-0 ${pickAlignClass(column)}`}
                title={column.description}
              >
                <Show
                  when={isSortable(column)}
                  fallback={
                    drawHeader ? drawHeader() : <span class="truncate">{column.label}</span>
                  }
                >
                  <button
                    type="button"
                    class={[
                      "-mx-1 flex cursor-pointer items-baseline gap-1 rounded px-1 uppercase hover:text-ink",
                      {
                        "text-ink": isSorted(),
                        "flex-row-reverse": resolveColumnAlign(column) === "end",
                      },
                    ]}
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
        keyed={props.rowKey}
        fallback={
          <div role="row" class="col-span-full px-3 py-8 text-center font-sans text-muted">
            <span role="cell">{props.emptyMessage ?? "Nothing to show."}</span>
          </div>
        }
      >
        {(row) => {
          const isSelected = () => selectedKeys[props.rowKey(row())] === true;
          return (
            <>
              <TableRow
                row={row()}
                columns={props.columns}
                largestMeterValues={largestMeterValues()}
                href={props.href?.(row())}
                onRowClick={props.onRowClick}
                selected={props.selectedKey ? isSelected() : undefined}
                tone={props.tone?.(row())}
                level={props.level?.(row())}
              />
              <Show when={props.detail && isSelected()}>
                <div role="row" class="col-span-full border-b border-line">
                  <div role="cell">{props.detail?.(row())}</div>
                </div>
              </Show>
            </>
          );
        }}
      </For>
    </div>
  );
}

function haveSameEntries(
  previous: ReadonlyMap<string, number>,
  next: ReadonlyMap<string, number>,
): boolean {
  if (previous.size !== next.size) return false;
  for (const [key, value] of next) if (previous.get(key) !== value) return false;
  return true;
}
