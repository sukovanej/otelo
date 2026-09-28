import { For } from "solid-js";
import { plain } from "./classes";

/** A row of tabs, one of them selected. */
export default function Tabs<T extends string>(props: {
  options: readonly { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
  label?: string;
}) {
  return (
    <div class="flex" role="tablist" aria-label={props.label}>
      <For each={props.options}>
        {(option) => (
          <button
            type="button"
            role="tab"
            aria-selected={props.value === option.value}
            class={`${plain} -ml-px h-6.5 cursor-pointer border px-2.5 text-muted first:ml-0 first:rounded-l-md last:rounded-r-md hover:bg-hover aria-selected:bg-active aria-selected:text-ink`}
            onClick={() => props.onChange(option.value)}
          >
            {option.label}
          </button>
        )}
      </For>
    </div>
  );
}
