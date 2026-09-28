import { createMemo, createSignal, For, type JSX, Show } from "solid-js";

import Meter from "./Meter";
import { sortRows, type SortValue } from "./sort";
import type { Unit } from "./units";
import Value from "./Value";

/** A state a cell or a row shows in its color. */
export type Tone = "error" | "warn" | "muted";

/** One column of a table. */
export interface Column<R> {
  id: string;
  label: string;
  /** What the column sorts by, and shows when it has no `cell`. */
  value: (row: R) => SortValue;
  /** The unit of a column of numbers, which sets them right-aligned. */
  unit?: Unit;
  /** What a cell shows in place of its value. */
  cell?: (row: R) => JSX.Element;
  /** What the header shows in place of the label, which stays its name for
   * screen readers. */
  header?: () => JSX.Element;
  /** A bar before each number of its share of the largest in the column. */
  meter?: boolean;
  /** Where the cells sit: at the end for a column with a unit, else at the
   * start. */
  align?: "start" | "end";
  /** The color of a cell that means a state, such as a count of errors. */
  tone?: (row: R) => Tone | undefined;
  /** Its track in the grid, such as `1fr` or `12ch`. A column of numbers
   * fits its widest cell, one with a meter takes a share of the room, and a
   * column of text takes the rest. */
  width?: string;
  /** What the header says on hover. */
  description?: string;
  /** A column sorts on a click on its header when the table sorts, unless
   * this is false. */
  sortable?: boolean;
}

export interface SortOrder {
  column: string;
  descending: boolean;
}

/** Where a row of a tree sits: how deep, and whether its children show. */
export interface TreeLevel {
  depth: number;
  /** `undefined` for a row without children. */
  expanded?: boolean | undefined;
}

const cellTones: Record<Tone, string> = {
  error: "text-error",
  warn: "text-warn",
  muted: "text-muted",
};

const track = <R,>(column: Column<R>) =>
  column.width ??
  (column.meter ? "minmax(16ch,1.2fr)" : column.unit ? "max-content" : "minmax(12ch,2fr)");

/** Whether a click on a link is a plain left click, which the table can take
 * instead of the browser. A click with a modifier key opens the address the
 * browser's way, such as in a new tab. */
const plainClick = (e: MouseEvent) =>
  e.button === 0 && !e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey;

/**
 * Rows of values under the names of their columns: the one table of the UI,
 * for lists and for tables of numbers alike.
 *
 * - The columns size to their content and every row lines up with them. The
 *   header stays at the top while the rows scroll.
 * - With `sort`, a click on the name of a column sorts the rows by it, the
 *   largest first, and a second click turns the order around. Without it,
 *   the rows keep their order.
 * - A row links to `href`, calls `onRowClick` on a click or on Enter, or
 *   both: then a plain click calls `onRowClick`, and a click with a modifier
 *   key opens the link the browser's way.
 * - `selected` marks the selected rows, `tone` tints a row, and `detail`
 *   shows under the rows that are `expanded`. With `level`, the table is a
 *   tree.
 * - While `loading`, the table keeps its rows, fainter.
 * - The header covers the rows under it in `--viz-surface`, which a panel
 *   sets to its color, or else in the color of the page.
 */
export default function Table<R>(props: {
  label: string;
  rows: R[];
  columns: Column<R>[];
  sort?: SortOrder;
  href?: (row: R) => string;
  onRowClick?: (row: R) => void;
  selected?: (row: R) => boolean;
  tone?: (row: R) => "error" | undefined;
  /** Whether the panel of `detail` shows under a row. */
  expanded?: (row: R) => boolean;
  detail?: (row: R) => JSX.Element;
  level?: (row: R) => TreeLevel;
  empty?: JSX.Element;
  loading?: boolean;
}) {
  const [order, setOrder] = createSignal<SortOrder | undefined>(props.sort);
  const sorts = (c: Column<R>) => props.sort !== undefined && c.sortable !== false;
  const sorted = createMemo(() => {
    const current = order();
    const by = props.columns.find((c) => c.id === current?.column);
    return current && by ? sortRows(props.rows, by.value, current.descending) : props.rows;
  });
  // The largest value of each column with a meter.
  const most = createMemo(() => {
    const out = new Map<string, number>();
    for (const c of props.columns) {
      if (!c.meter) continue;
      const values = props.rows.map((row) => c.value(row)).filter((v) => typeof v === "number");
      out.set(c.id, Math.max(0, ...values));
    }
    return out;
  });
  const toggle = (id: string) =>
    setOrder((current) =>
      current?.column === id
        ? { column: id, descending: !current.descending }
        : { column: id, descending: true },
    );

  const align = (c: Column<R>) =>
    (c.align ?? (c.unit ? "end" : "start")) === "end" ? "justify-end text-right" : "";

  const content = (c: Column<R>, row: R) => {
    if (c.cell) return c.cell(row);
    const value = c.value(row);
    if (!c.unit) return <span class="truncate">{value ?? "–"}</span>;
    const number = typeof value === "number" ? value : null;
    const shown = <Value value={number} unit={c.unit} align />;
    if (!c.meter) return shown;
    const top = most().get(c.id) ?? 0;
    return <Meter share={top > 0 && number !== null ? number / top : 0}>{shown}</Meter>;
  };

  const rowClass = (row: R) => {
    const interactive = props.href || props.onRowClick;
    const state = props.selected?.(row)
      ? "bg-active shadow-[inset_3px_0_0_var(--color-accent)]"
      : props.tone?.(row) === "error"
        ? `bg-error/5 ${interactive ? "hover:bg-error/10" : ""}`
        : interactive
          ? "hover:bg-hover"
          : "";
    return `col-span-full grid grid-cols-subgrid items-center border-b border-line px-3 py-1.5 ${
      interactive ? "cursor-pointer" : ""
    } ${state}`;
  };

  const Cells = (cellProps: { row: R }) => (
    <For each={props.columns}>
      {(c) => {
        const tone = () => {
          const name = c.tone?.(cellProps.row);
          return name ? cellTones[name] : "";
        };
        return (
          <div role="cell" class={`flex min-w-0 items-baseline ${align(c)} ${tone()}`}>
            {content(c, cellProps.row)}
          </div>
        );
      }}
    </For>
  );

  const Row = (rowProps: { row: R }) => {
    const row = rowProps.row;
    const aria = () => {
      const level = props.level?.(row);
      return {
        "aria-selected": props.selected?.(row),
        "aria-level": level ? level.depth + 1 : undefined,
        "aria-expanded": level?.expanded,
      };
    };
    const click = (e: MouseEvent) => {
      if (!props.onRowClick) return;
      if (props.href) {
        if (!plainClick(e)) return;
        e.preventDefault();
      }
      // A click that ends a text selection is not a click on the row.
      if (window.getSelection()?.isCollapsed === false) return;
      props.onRowClick(row);
    };
    return (
      <Show
        when={props.href}
        fallback={
          <div
            role="row"
            class={rowClass(row)}
            tabIndex={props.onRowClick ? 0 : undefined}
            onClick={click}
            onKeyDown={(e) => {
              if (props.onRowClick && (e.key === "Enter" || e.key === " ")) {
                e.preventDefault();
                props.onRowClick(row);
              }
            }}
            {...aria()}
          >
            <Cells row={row} />
          </div>
        }
      >
        {(href) => (
          <a
            role="row"
            href={href()(row)}
            class={`${rowClass(row)} text-ink`}
            onClick={click}
            {...aria()}
          >
            <Cells row={row} />
          </a>
        )}
      </Show>
    );
  };

  return (
    <div
      role={props.level ? "treegrid" : "table"}
      aria-label={props.label}
      aria-busy={props.loading}
      class="grid gap-x-3 font-mono text-sm transition-opacity"
      classList={{ "opacity-60": props.loading }}
      style={{ "grid-template-columns": props.columns.map(track).join(" ") }}
    >
      <div
        role="row"
        class="sticky top-0 z-10 col-span-full grid grid-cols-subgrid items-center border-b border-line bg-(--viz-surface,var(--color-surface)) px-3 py-2 font-sans text-2xs font-semibold tracking-[0.04em] text-muted uppercase"
      >
        <For each={props.columns}>
          {(c) => {
            const active = () => order()?.column === c.id;
            return (
              <div
                role="columnheader"
                aria-sort={
                  active() ? (order()?.descending ? "descending" : "ascending") : undefined
                }
                aria-label={c.header ? c.label : undefined}
                class={`flex min-w-0 ${align(c)}`}
                title={c.description}
              >
                <Show
                  when={sorts(c)}
                  fallback={c.header ? c.header() : <span class="truncate">{c.label}</span>}
                >
                  <button
                    type="button"
                    class="-mx-1 flex cursor-pointer items-baseline gap-1 rounded px-1 uppercase hover:text-ink"
                    classList={{ "text-ink": active(), "flex-row-reverse": align(c) !== "" }}
                    onClick={() => toggle(c.id)}
                  >
                    {c.label}
                    <span class={active() ? "" : "invisible"} aria-hidden="true">
                      {order()?.descending ? "↓" : "↑"}
                    </span>
                  </button>
                </Show>
              </div>
            );
          }}
        </For>
      </div>

      <For
        each={sorted()}
        fallback={
          <div role="row" class="col-span-full px-3 py-8 text-center font-sans text-muted">
            <span role="cell">{props.empty ?? "Nothing to show."}</span>
          </div>
        }
      >
        {(row) => (
          <>
            <Row row={row} />
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
