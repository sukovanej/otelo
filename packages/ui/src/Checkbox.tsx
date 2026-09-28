import type { JSX } from "solid-js";

/** A checkbox with its label. */
export default function Checkbox(props: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  title?: string;
  children: JSX.Element;
}) {
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
