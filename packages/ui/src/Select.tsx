import { For } from "solid-js";

import { control, cx, plain, type Size, sizes } from "./classes";

interface SelectOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

interface SelectProps<T extends string> {
  readonly label: string;
  readonly options: ReadonlyArray<SelectOption<T>>;
  readonly value: T;
  readonly onChange: (value: T) => void;
  readonly size?: Size | undefined;
}

export default function Select<T extends string>(props: SelectProps<T>) {
  return (
    <label class="flex items-center gap-2">
      <span class="text-muted">{props.label}</span>
      <select
        class={cx(control, plain, sizes[props.size ?? "md"], "cursor-pointer hover:bg-hover")}
        value={props.value}
        onChange={(e) => {
          const picked = props.options.find((choice) => choice.value === e.currentTarget.value);
          if (picked) props.onChange(picked.value);
        }}
      >
        <For each={props.options}>
          {(choice) => <option value={choice.value}>{choice.label}</option>}
        </For>
      </select>
    </label>
  );
}
