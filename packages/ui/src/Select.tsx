import { CheckIcon, ChevronIcon } from "@siner/icons";
import { createSignal, createUniqueId, For, Show } from "solid-js";
import {
  activeOption,
  control,
  cx,
  option as optionClass,
  plain,
  popup,
  type Size,
  sizes,
} from "./classes";
import { listStep, move } from "./keys";

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

/** How long a pause ends the letters typed to find an option. */
const TYPEAHEAD_MS = 600;

/**
 * A choice of one option from a list that opens under a button. Enter, Space,
 * or the arrow keys open it; the arrow keys, Ctrl+J and Ctrl+K, Home and End,
 * or the first letters of a label move through it; Enter, Space, or Tab takes
 * the option; Escape closes it.
 */
export default function Select<T extends string>(props: {
  options: readonly SelectOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /** The name of the choice, for screen readers. */
  label: string;
  size?: Size;
}) {
  const id = createUniqueId();
  let list: HTMLUListElement | undefined;
  const [open, setOpen] = createSignal(false);
  const [active, setActive] = createSignal(0);
  let typed = "";
  let typedAt = 0;

  const index = () =>
    Math.max(
      0,
      props.options.findIndex((option) => option.value === props.value),
    );
  const current = () => props.options[index()];

  const reveal = () => list?.querySelector("[data-active]")?.scrollIntoView({ block: "nearest" });
  const show = () => {
    setActive(index());
    setOpen(true);
    reveal();
  };
  const choose = (i: number) => {
    const option = props.options[i];
    setOpen(false);
    if (option && option.value !== props.value) props.onChange(option.value);
  };
  const highlight = (i: number) => {
    setActive(i);
    reveal();
  };

  /** Moves to the next option whose label starts with the letters typed. */
  const find = (letter: string) => {
    const now = Date.now();
    typed = now - typedAt > TYPEAHEAD_MS ? letter : typed + letter;
    typedAt = now;
    const count = props.options.length;
    // A repeated single letter cycles through the options it starts.
    const from = typed.length === 1 ? active() + 1 : active();
    for (let k = 0; k < count; k++) {
      const i = (from + k) % count;
      if (props.options[i]?.label.toLowerCase().startsWith(typed.toLowerCase())) {
        if (open()) highlight(i);
        else choose(i);
        return;
      }
    }
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const step = listStep(e);
    if (step !== 0) {
      e.preventDefault();
      if (open()) highlight(move(active(), step, props.options.length));
      else show();
      return;
    }
    switch (e.key) {
      case "Enter":
      case " ":
        e.preventDefault();
        if (open()) choose(active());
        else show();
        return;
      case "Tab":
        if (open()) choose(active());
        return;
      case "Escape":
        if (open()) {
          e.preventDefault();
          setOpen(false);
        }
        return;
      case "Home":
      case "End":
        if (open()) {
          e.preventDefault();
          highlight(e.key === "Home" ? 0 : props.options.length - 1);
        }
        return;
      default:
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) find(e.key);
    }
  };

  return (
    <div class="relative">
      <button
        type="button"
        class={cx(
          control,
          plain,
          sizes[props.size ?? "md"],
          "group flex min-w-[11ch] cursor-pointer items-center gap-2 text-left hover:bg-hover",
        )}
        role="combobox"
        aria-label={props.label}
        aria-haspopup="listbox"
        aria-expanded={open()}
        aria-controls={`${id}-list`}
        aria-activedescendant={open() ? `${id}-${active()}` : undefined}
        onClick={() => (open() ? setOpen(false) : show())}
        onKeyDown={onKeyDown}
        onBlur={() => setOpen(false)}
      >
        <span class="flex-1 whitespace-nowrap">{current()?.label}</span>
        <ChevronIcon size={13} class="text-muted group-aria-expanded:rotate-180" />
      </button>
      <Show when={open()}>
        <ul
          ref={list}
          class={cx(popup, "left-0 w-max min-w-full")}
          id={`${id}-list`}
          role="listbox"
          aria-label={props.label}
        >
          <For each={props.options}>
            {(option, i) => (
              <li
                id={`${id}-${i()}`}
                role="option"
                aria-selected={option.value === props.value}
                data-active={active() === i() ? "" : undefined}
                class={cx(
                  optionClass,
                  "items-center gap-2 whitespace-nowrap pr-4",
                  active() === i() && activeOption,
                )}
                // Keep the focus on the button.
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => choose(i())}
                onMouseEnter={() => setActive(i())}
              >
                <CheckIcon
                  size={13}
                  class={cx("text-accent", option.value !== props.value && "invisible")}
                />
                {option.label}
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
