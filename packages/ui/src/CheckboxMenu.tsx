import { For, Show } from "solid-js";

import { ChevronIcon } from "@otelo/icons";

import { control, cx, option, plain, popover, type Size, sizes } from "./classes";
import { createPopover } from "./popover";

interface CheckboxMenuOption {
  readonly value: string;
  readonly label: string;
}

interface CheckboxMenuSection {
  readonly title: string;
  readonly options: ReadonlyArray<CheckboxMenuOption>;
}

interface CheckboxMenuProps {
  readonly label: string;
  readonly placeholder: string;
  readonly sections: ReadonlyArray<CheckboxMenuSection>;
  readonly checked: ReadonlyArray<string>;
  readonly onChange: (checked: string[]) => void;
  readonly size?: Size | undefined;
}

export default function CheckboxMenu(props: CheckboxMenuProps) {
  let trigger!: HTMLButtonElement;
  const menuPopover = createPopover({ returnFocusTo: () => trigger });
  const labelOf = (value: string) =>
    props.sections
      .flatMap((section) => section.options)
      .find((sectionOption) => sectionOption.value === value)?.label ?? value;
  const toggleValue = (value: string, isChecked: boolean) =>
    props.onChange(
      isChecked
        ? [...props.checked.filter((checkedValue) => checkedValue !== value), value]
        : props.checked.filter((checkedValue) => checkedValue !== value),
    );

  return (
    <div ref={menuPopover.rootRef} class="relative">
      <button
        ref={trigger}
        type="button"
        class={cx(
          control,
          plain,
          sizes[props.size ?? "md"],
          "group flex max-w-96 cursor-pointer items-center gap-2 hover:bg-hover",
        )}
        aria-haspopup="dialog"
        aria-expanded={menuPopover.open() ? "true" : "false"}
        onClick={() => (menuPopover.open() ? menuPopover.hide() : menuPopover.show())}
      >
        <span class="text-muted">{props.label}</span>
        <span class={["truncate", { "text-muted": props.checked.length === 0 }]}>
          {props.checked.length === 0
            ? props.placeholder
            : props.checked.map((value) => labelOf(value)).join(", ")}
        </span>
        <ChevronIcon size={13} class="text-muted group-aria-expanded:rotate-180" />
      </button>
      <Show when={menuPopover.open()}>
        <div
          ref={menuPopover.panelRef}
          class={cx(
            popover,
            "max-h-96 min-w-56 overflow-y-auto p-1",
            menuPopover.alignsRight() ? "right-0" : "left-0",
          )}
          role="dialog"
          aria-label={props.label}
        >
          <For each={props.sections.filter((section) => section.options.length > 0)} keyed={false}>
            {(section) => (
              <fieldset class="m-0 border-0 p-0">
                <legend class="px-2 pt-1.5 pb-0.5 text-2xs font-semibold tracking-[0.04em] text-muted uppercase">
                  {section().title}
                </legend>
                <For each={section().options} keyed={false}>
                  {(sectionOption) => (
                    <label
                      class={cx(option, "items-center gap-2 whitespace-nowrap hover:bg-active")}
                    >
                      <input
                        type="checkbox"
                        class="accent-accent"
                        checked={props.checked.includes(sectionOption().value)}
                        onChange={(e) =>
                          toggleValue(sectionOption().value, e.currentTarget.checked)
                        }
                      />
                      <span class="font-mono">{sectionOption().label}</span>
                    </label>
                  )}
                </For>
              </fieldset>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
