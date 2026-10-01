import { Show } from "solid-js";

import { CalendarIcon } from "@otelo/icons";

import { control, cx, plain, popover, sizes } from "../classes";
import { formatDate, parseDate } from "../date";
import { createPopover } from "../popover";
import Calendar from "./calendar";

interface DateInputProps {
  readonly value: string;
  readonly onChange: (date: string) => void;
  readonly label: string;
}

export default function DateInput(props: DateInputProps) {
  let input!: HTMLInputElement;
  const calendarPopover = createPopover({ returnFocusTo: () => input });

  const takeDate = (date: string) => {
    input.value = formatDate(date);
    if (date !== props.value) props.onChange(date);
  };
  const takeTypedDate = () => takeDate(parseDate(input.value) ?? props.value);
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      // Not the form around the field: Enter takes the date.
      e.preventDefault();
      takeTypedDate();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      calendarPopover.show();
    }
  };

  return (
    <div ref={calendarPopover.rootRef} class="relative">
      <div
        class={cx(
          control,
          plain,
          sizes.md,
          "flex items-center gap-1.5 focus-within:border-line-focus",
        )}
      >
        <button
          type="button"
          class="cursor-pointer text-muted hover:text-ink"
          tabindex={-1}
          aria-label={`Choose ${props.label.toLowerCase()} from a calendar`}
          aria-expanded={calendarPopover.open() ? "true" : "false"}
          onClick={() => (calendarPopover.open() ? calendarPopover.hide() : calendarPopover.show())}
        >
          <CalendarIcon size={14} />
        </button>
        <input
          ref={input}
          class="w-[12ch] bg-transparent font-mono focus:outline-none"
          aria-label={props.label}
          placeholder="Sep 30, 2026"
          autocomplete="off"
          value={formatDate(props.value)}
          onChange={takeTypedDate}
          onClick={() => calendarPopover.show()}
          onKeyDown={onKeyDown}
        />
      </div>
      <Show when={calendarPopover.open()}>
        <div
          ref={calendarPopover.panelRef}
          class={cx(popover, calendarPopover.alignsRight() ? "right-0" : "left-0")}
        >
          <Calendar
            label={props.label}
            value={props.value}
            onChange={(date) => {
              calendarPopover.hide();
              input.focus();
              takeDate(date);
            }}
          />
        </div>
      </Show>
    </div>
  );
}
