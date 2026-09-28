import { For } from "solid-js";
import { durationParts, formatDuration } from "./time";

/**
 * A duration such as `4.56 ms`: each number bold in the color of the text
 * around it, and its unit fainter after a gap. In a column, `align` sets the last unit in
 * a box of two characters, so the numbers of right-aligned rows line up.
 */
export default function Duration(props: { nanos: number; align?: boolean }) {
  const parts = () => durationParts(props.nanos);
  return (
    <span class="whitespace-nowrap" title={formatDuration(props.nanos)}>
      <For each={parts()}>
        {(part, i) => (
          <>
            <span class="font-semibold">{part.value}</span>
            <span
              class={`ml-[0.3ch] font-normal opacity-60 ${
                props.align && i() === parts().length - 1 ? "inline-block w-[2ch] text-left" : ""
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
