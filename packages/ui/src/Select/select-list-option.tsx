import { CheckIcon } from "@otelo/icons";

import { cx, option } from "../classes";
import type { SelectOption } from "./select-options";

interface SelectListOptionProps<T extends string> {
  readonly option: SelectOption<T>;
  readonly chosen: boolean;
  readonly onPick: () => void;
}

export default function SelectListOption<T extends string>(props: SelectListOptionProps<T>) {
  return (
    <div
      role="option"
      aria-selected={props.chosen ? "true" : "false"}
      data-value={props.option.value}
      tabindex={-1}
      class={cx(
        option,
        "items-center gap-2 pr-4 whitespace-nowrap outline-none hover:bg-active focus:bg-active",
      )}
      onClick={() => props.onPick()}
    >
      <CheckIcon size={13} class={cx("shrink-0 text-accent", !props.chosen && "invisible")} />
      {props.option.label}
    </div>
  );
}
