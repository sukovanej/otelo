import { Show } from "solid-js";

import { isPlainLeftClick } from "@otelo/ui";

import type { Column } from "../column";
import TableCells from "./table-cells";

export type RowTone = "error";

export type TreeLevel = TreeLeaf | TreeBranch;

interface TreeLevelBase {
  readonly depth: number;
}

interface TreeLeaf extends TreeLevelBase {
  readonly kind: "leaf";
}

interface TreeBranch extends TreeLevelBase {
  readonly kind: "branch";
  readonly expanded: boolean;
}

interface TableRowProps<R> {
  readonly row: R;
  readonly columns: ReadonlyArray<Column<R>>;
  readonly largestMeterValues: ReadonlyMap<string, number>;
  readonly href: string | undefined;
  readonly onRowClick: ((row: R) => void) | undefined;
  readonly selected: boolean | undefined;
  readonly tone: RowTone | undefined;
  readonly level: TreeLevel | undefined;
}

export default function TableRow<R>(props: TableRowProps<R>) {
  const ariaAttributes = () => {
    const level = props.level;
    return {
      "aria-selected": toAriaBoolean(props.selected),
      "aria-level": level ? level.depth + 1 : undefined,
      "aria-expanded": toAriaBoolean(level?.kind === "branch" ? level.expanded : undefined),
    };
  };
  const onClick = (e: MouseEvent) => {
    if (!props.onRowClick) return;
    if (props.href !== undefined) {
      // A click with a modifier key opens the link the browser's way.
      if (!isPlainLeftClick(e)) return;
      e.preventDefault();
    }
    // A click that ends a text selection is not a click on the row.
    if (window.getSelection()?.isCollapsed === false) return;
    props.onRowClick(props.row);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (props.onRowClick && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      props.onRowClick(props.row);
    }
  };
  return (
    <Show
      when={props.href !== undefined}
      fallback={
        <div
          role="row"
          class={pickRowClasses(props)}
          tabindex={props.onRowClick ? 0 : undefined}
          onClick={onClick}
          onKeyDown={onKeyDown}
          {...ariaAttributes()}
        >
          <TableCells
            row={props.row}
            columns={props.columns}
            largestMeterValues={props.largestMeterValues}
          />
        </div>
      }
    >
      <a
        role="row"
        href={props.href}
        class={`${pickRowClasses(props)} text-ink`}
        onClick={onClick}
        {...ariaAttributes()}
      >
        <TableCells
          row={props.row}
          columns={props.columns}
          largestMeterValues={props.largestMeterValues}
        />
      </a>
    </Show>
  );
}

function pickRowClasses<R>(props: TableRowProps<R>): string {
  const isInteractive = props.href !== undefined || props.onRowClick !== undefined;
  const stateClasses = props.selected
    ? "bg-active shadow-[inset_3px_0_0_var(--color-accent)]"
    : props.tone === "error"
      ? `bg-error/5 ${isInteractive ? "hover:bg-error/10" : ""}`
      : isInteractive
        ? "hover:bg-hover"
        : "";
  return `col-span-full grid grid-cols-subgrid items-center border-b border-line px-3 py-1.5 ${
    isInteractive ? "cursor-pointer" : ""
  } ${stateClasses}`;
}

function toAriaBoolean(value: boolean | undefined): "true" | "false" | undefined {
  if (value === undefined) return undefined;
  return value ? "true" : "false";
}
