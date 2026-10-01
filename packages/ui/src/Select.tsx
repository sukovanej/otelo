import { flush, For, Show } from "solid-js";

import { CheckIcon, ChevronIcon } from "@otelo/icons";

import { control, cx, option, plain, popup, type Size, sizes } from "./classes";
import { moveListIndex, toListStep } from "./keys";
import { createPopover } from "./popover";

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
  let trigger!: HTMLButtonElement;
  let optionList!: HTMLUListElement;
  const listPopover = createPopover({ returnFocusTo: () => trigger });
  const selectedIndex = () => props.options.findIndex((choice) => choice.value === props.value);
  const listOptionElements = () => [...optionList.querySelectorAll<HTMLElement>("[role=option]")];

  const openList = () => {
    listPopover.show();
    flush();
    listOptionElements()[Math.max(0, selectedIndex())]?.focus();
  };
  const pickValue = (value: T) => {
    listPopover.hide();
    trigger.focus();
    if (value !== props.value) props.onChange(value);
  };
  const onTriggerKeyDown = (e: KeyboardEvent) => {
    if (toListStep(e) === 0 || listPopover.open()) return;
    e.preventDefault();
    openList();
  };
  const onListKeyDown = (e: KeyboardEvent) => {
    const optionElements = listOptionElements();
    const focusedIndex = optionElements.findIndex((element) => element === document.activeElement);
    const step = toListStep(e);
    if (step !== 0) {
      e.preventDefault();
      optionElements[moveListIndex(focusedIndex, step, optionElements.length)]?.focus();
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      optionElements[e.key === "Home" ? 0 : optionElements.length - 1]?.focus();
    } else if (e.key === "Enter" || e.key === " ") {
      const focusedChoice = props.options[focusedIndex];
      if (!focusedChoice) return;
      e.preventDefault();
      pickValue(focusedChoice.value);
    }
  };

  return (
    <div ref={listPopover.rootRef} class="relative">
      <button
        ref={trigger}
        type="button"
        class={cx(
          control,
          plain,
          sizes[props.size ?? "md"],
          "group flex cursor-pointer items-center gap-2 whitespace-nowrap hover:bg-hover",
        )}
        aria-haspopup="listbox"
        aria-expanded={listPopover.open() ? "true" : "false"}
        onClick={() => (listPopover.open() ? listPopover.hide() : openList())}
        onKeyDown={onTriggerKeyDown}
      >
        <span class="text-muted">{props.label}</span>
        {props.options[selectedIndex()]?.label ?? props.value}
        <ChevronIcon size={13} class="text-muted group-aria-expanded:rotate-180" />
      </button>
      <Show when={listPopover.open()}>
        <ul
          ref={(list) => {
            optionList = list;
            listPopover.panelRef(list);
          }}
          class={cx(popup, "min-w-full", listPopover.alignsRight() ? "right-0" : "left-0")}
          role="listbox"
          aria-label={props.label}
          onKeyDown={onListKeyDown}
        >
          <For each={props.options}>
            {(choice) => (
              <li
                role="option"
                aria-selected={choice.value === props.value ? "true" : "false"}
                tabindex={-1}
                class={cx(
                  option,
                  "items-center gap-2 pr-4 whitespace-nowrap outline-none hover:bg-active focus:bg-active",
                )}
                onClick={() => pickValue(choice.value)}
              >
                <CheckIcon
                  size={13}
                  class={cx("text-accent", choice.value !== props.value && "invisible")}
                />
                {choice.label}
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
