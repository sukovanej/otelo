import type { JSX } from "@solidjs/web";
import { For } from "solid-js";

import { plain } from "./classes";

export interface TabOption<T extends string> {
  readonly value: T;
  readonly label: string;
  readonly icon?: TabIcon;
}

type TabIcon = (props: TabIconProps) => JSX.Element;

interface TabIconProps {
  readonly size: number;
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
            class={`${plain} -ml-px flex h-7 cursor-pointer items-center gap-1.5 border px-2.5 text-muted first:ml-0 first:rounded-l-md last:rounded-r-md hover:bg-hover aria-selected:bg-active aria-selected:text-ink`}
            onClick={() => props.onChange(option.value)}
          >
            {option.icon?.({ size: 13 })}
            {option.label}
          </button>
        )}
      </For>
    </div>
  );
}
