import { Show } from "solid-js";

import { CalendarIcon } from "@otelo/icons";

import Calendar from "./Calendar";
import { control, cx, plain, popover as popoverClass, type Size, sizes } from "./classes";
import { formatDate, parseDate } from "./date";
import { createPopover } from "./popover";

/**
 * A date, as `2026-09-30`, which the field shows as `Sep 30, 2026`. It is
 * typed either way, or chosen from a calendar that opens under the field on
 * a click or the down arrow. Enter or leaving the field takes what is typed,
 * and text that is no date goes back to the date.
 */
export default function DateInput(props: {
  value: string;
  onChange: (date: string) => void;
  /** The first and the last date to choose, when the choice has such. */
  min?: string | undefined;
  max?: string | undefined;
  /** The name of the date, for screen readers. */
  label: string;
  size?: Size | undefined;
}) {
  let input!: HTMLInputElement;
  const popover = createPopover({ focus: () => input });

  const allowed = (date: string) =>
    (props.min === undefined || date >= props.min) &&
    (props.max === undefined || date <= props.max);
  const take = (date: string) => {
    input.value = formatDate(date);
    if (date !== props.value) props.onChange(date);
  };
  const commit = () => {
    const date = parseDate(input.value);
    take(date !== undefined && allowed(date) ? date : props.value);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      // Not the form around the field: Enter takes the date.
      e.preventDefault();
      commit();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      popover.show();
    }
  };

  return (
    <div ref={popover.root} class="relative">
      <div
        class={cx(
          control,
          plain,
          sizes[props.size ?? "md"],
          "flex items-center gap-1.5 focus-within:border-line-focus",
        )}
      >
        <button
          type="button"
          class="cursor-pointer text-muted hover:text-ink"
          tabindex={-1}
          aria-label={`Choose ${props.label.toLowerCase()} from a calendar`}
          aria-expanded={popover.open()}
          onClick={() => (popover.open() ? popover.hide() : popover.show())}
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
          onChange={commit}
          onClick={() => popover.show()}
          onKeyDown={onKeyDown}
        />
      </div>
      <Show when={popover.open()}>
        <div ref={popover.panel} class={cx(popoverClass, popover.atEnd() ? "right-0" : "left-0")}>
          <Calendar
            label={props.label}
            value={props.value}
            min={props.min}
            max={props.max}
            onChange={(date) => {
              popover.hide();
              input.focus();
              take(date);
            }}
          />
        </div>
      </Show>
    </div>
  );
}
