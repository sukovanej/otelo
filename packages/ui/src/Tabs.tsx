import { For } from "solid-js";

import { plain } from "./classes";

interface TabOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

interface TabsProps<T extends string> {
  readonly options: ReadonlyArray<TabOption<T>>;
  readonly value: T;
  readonly onChange: (value: T) => void;
  readonly label: string;
}

export default function Tabs<T extends string>(props: TabsProps<T>) {
  return (
    <div class="flex" role="tablist" aria-label={props.label}>
      <For each={props.options}>
        {(option) => (
          <button
            type="button"
            role="tab"
            aria-selected={props.value === option.value ? "true" : "false"}
            class={`${plain} -ml-px h-7 cursor-pointer border px-2.5 text-muted first:ml-0 first:rounded-l-md last:rounded-r-md hover:bg-hover aria-selected:bg-active aria-selected:text-ink`}
            onClick={() => props.onChange(option.value)}
          >
            {option.label}
          </button>
        )}
      </For>
    </div>
  );
}
