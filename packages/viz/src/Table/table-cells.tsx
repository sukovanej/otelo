import type { JSX } from "@solidjs/web";
import { For } from "solid-js";

import { type CellTone, type Column, pickAlignClass } from "../column";
import Value from "../Value";
import TableMeter from "./table-meter";

const CELL_TONE_CLASSES: Record<CellTone, string> = {
  error: "text-error",
  warn: "text-warn",
  muted: "text-muted",
};

interface TableCellsProps<R> {
  readonly row: R;
  readonly columns: ReadonlyArray<Column<R>>;
  readonly largestMeterValues: ReadonlyMap<string, number>;
}

export default function TableCells<R>(props: TableCellsProps<R>) {
  return (
    <For each={props.columns}>
      {(column) => {
        const toneClass = () => {
          const tone = column.tone?.(props.row);
          return tone ? CELL_TONE_CLASSES[tone] : "";
        };
        const drawContent = (): JSX.Element => {
          if (column.kind === "cell") return column.cell(props.row);
          if (column.kind === "text") {
            return <span class="truncate">{column.value(props.row) ?? "–"}</span>;
          }
          const value = column.value(props.row);
          const valueElement = <Value value={value} unit={column.unit} inColumn />;
          if (column.kind === "number") return valueElement;
          const largest = props.largestMeterValues.get(column.id) ?? 0;
          return (
            <TableMeter share={largest > 0 && value !== null ? value / largest : 0}>
              {valueElement}
            </TableMeter>
          );
        };
        return (
          <div
            role="cell"
            class={`flex min-w-0 items-baseline ${pickAlignClass(column)} ${toneClass()}`}
          >
            {drawContent()}
          </div>
        );
      }}
    </For>
  );
}
