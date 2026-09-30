import type { JSX } from "solid-js";

interface CheckboxProps {
  readonly checked: boolean;
  readonly onChange: (checked: boolean) => void;
  readonly disabled?: boolean;
  readonly title?: string;
  readonly children: JSX.Element;
}

export default function Checkbox(props: CheckboxProps) {
  return (
    <label class="flex cursor-pointer items-center gap-1" title={props.title}>
      <input
        type="checkbox"
        class="accent-accent"
        checked={props.checked}
        disabled={props.disabled}
        onChange={(e) => props.onChange(e.currentTarget.checked)}
      />
      {props.children}
    </label>
  );
}
