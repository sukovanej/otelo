import { For } from "solid-js";

import { formatValue, splitValue, type Unit } from "./units";

interface ValueProps {
  readonly value: number | null | undefined;
  readonly unit: Unit;
  readonly inColumn?: boolean;
}

export default function Value(props: ValueProps) {
  const valueParts = () =>
    props.value === null || props.value === undefined ? [] : splitValue(props.value, props.unit);
  return (
    <span
      class={`whitespace-nowrap ${props.inColumn ? "tabular-nums" : ""}`}
      title={formatValue(props.value, props.unit)}
    >
      <For each={valueParts()} fallback={<span class="opacity-50">–</span>}>
        {(part, index) => (
          <>
            <span class="font-semibold">{part.value}</span>
            <span
              // The last unit takes two characters at least, so the numbers of
              // right-aligned rows line up.
              class={`${part.unitAttached ? "" : "ml-[0.3ch]"} font-normal opacity-60 ${
                props.inColumn && index() === valueParts().length - 1
                  ? "inline-block min-w-[2ch] text-left"
                  : ""
              } ${index() < valueParts().length - 1 ? "mr-[0.5ch]" : ""}`}
            >
              {part.unit}
            </span>
          </>
        )}
      </For>
    </span>
  );
}
