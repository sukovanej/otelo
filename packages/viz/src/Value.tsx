import { For } from "solid-js";

import { formatValue, type Unit, valueParts } from "./units";

/**
 * A value in its unit, such as `4.56 ms`: each number bold, and its unit
 * fainter after a gap, or right after it for `%`, `/s`, and `K`. A missing
 * value is a faint dash. In a column, `align` gives the digits one width and
 * sets the last unit in a box at least two characters wide, so the numbers of
 * right-aligned rows line up. Without it, a large number keeps its natural
 * figures.
 */
export default function Value(props: {
  value: number | null | undefined;
  unit: Unit;
  align?: boolean;
  /** Keep the weight of the text around it. */
  plain?: boolean;
}) {
  const parts = () =>
    props.value === null || props.value === undefined ? [] : valueParts(props.value, props.unit);
  return (
    <span
      class={`whitespace-nowrap ${props.align ? "tabular-nums" : ""}`}
      title={formatValue(props.value, props.unit)}
    >
      <For each={parts()} fallback={<span class="opacity-50">–</span>}>
        {(part, i) => (
          <>
            <span class={props.plain ? "" : "font-semibold"}>{part.value}</span>
            <span
              class={`${part.tight ? "" : "ml-[0.3ch]"} font-normal opacity-60 ${
                props.align && i() === parts().length - 1
                  ? "inline-block min-w-[2ch] text-left"
                  : ""
              } ${i() < parts().length - 1 ? "mr-[0.5ch]" : ""}`}
            >
              {part.unit}
            </span>
          </>
        )}
      </For>
    </span>
  );
}
