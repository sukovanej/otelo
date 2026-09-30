import { createEffect, createMemo, createSignal, For, on } from "solid-js";

import { ChevronIcon } from "@otelo/icons";

import { cx } from "./classes";
import { addDays, addMonths, dateOf, MONTHS, monthWeeks, sameMonth, weekday } from "./date";

const WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/** Where each key moves the day that has the focus. */
const MOVES: Record<string, (date: string) => string> = {
  ArrowLeft: (date) => addDays(date, -1),
  ArrowRight: (date) => addDays(date, 1),
  ArrowUp: (date) => addDays(date, -7),
  ArrowDown: (date) => addDays(date, 7),
  Home: (date) => addDays(date, -weekday(date)),
  End: (date) => addDays(date, 6 - weekday(date)),
  PageUp: (date) => addMonths(date, -1),
  PageDown: (date) => addMonths(date, 1),
};

const monthButton =
  "flex size-7 cursor-pointer items-center justify-center rounded-md hover:bg-hover";

/**
 * A month of days to choose one date from, as `2026-09-30`. The arrow keys
 * move by a day and a week, Home and End to the ends of the week, and Page Up
 * and Page Down by a month; Enter or Space takes the day.
 */
export default function Calendar(props: {
  value: string | undefined;
  onChange: (date: string) => void;
  /** The first and the last date to choose, when the choice has such. */
  min?: string | undefined;
  max?: string | undefined;
  /** The name of the date, for screen readers. */
  label: string;
}) {
  let days!: HTMLDivElement;
  const today = dateOf(new Date());
  // The day that the keys move, which the calendar shows the month of.
  const [cursor, setCursor] = createSignal(props.value ?? today);
  createEffect(
    on(
      () => props.value,
      (value) => {
        if (value !== undefined) setCursor(value);
      },
      { defer: true },
    ),
  );

  const month = createMemo(() => cursor().slice(0, 7));
  const weeks = createMemo(() => monthWeeks(month()));
  const title = () => `${MONTHS[Number(month().slice(5)) - 1] ?? ""} ${month().slice(0, 4)}`;
  const allowed = (date: string) =>
    (props.min === undefined || date >= props.min) &&
    (props.max === undefined || date <= props.max);
  const name = (date: string) =>
    `${WEEKDAYS[weekday(date)] ?? ""} ${Number(date.slice(8))} ${MONTHS[Number(date.slice(5, 7)) - 1] ?? ""} ${date.slice(0, 4)}`;

  const onKeyDown = (e: KeyboardEvent) => {
    const to = MOVES[e.key];
    if (!to) return;
    e.preventDefault();
    const date = to(cursor());
    if (!allowed(date)) return;
    setCursor(date);
    days.querySelector<HTMLElement>("[tabindex='0']")?.focus();
  };

  return (
    <div class="w-max p-2 select-none" role="group" aria-label={props.label}>
      <div class="flex items-center justify-between pb-1">
        <button
          type="button"
          class={monthButton}
          aria-label="Previous month"
          onClick={() => setCursor(addMonths(cursor(), -1))}
        >
          <ChevronIcon direction="left" size={13} />
        </button>
        <span class="font-medium" aria-live="polite">
          {title()}
        </span>
        <button
          type="button"
          class={monthButton}
          aria-label="Next month"
          onClick={() => setCursor(addMonths(cursor(), 1))}
        >
          <ChevronIcon direction="right" size={13} />
        </button>
      </div>
      <div class="grid grid-cols-7 text-center text-2xs text-muted" aria-hidden="true">
        <For each={WEEKDAYS}>{(day) => <span class="py-1">{day.slice(0, 2)}</span>}</For>
      </div>
      <div ref={days} onKeyDown={onKeyDown}>
        <For each={weeks()}>
          {(week) => (
            <div class="grid grid-cols-7">
              <For each={week}>
                {(date) => (
                  <button
                    type="button"
                    class={cx(
                      "h-7 w-8 cursor-pointer rounded-md font-mono text-2xs disabled:cursor-default disabled:opacity-40",
                      date === props.value
                        ? "bg-accent font-semibold text-on-accent"
                        : "enabled:hover:bg-hover",
                      date !== props.value && date === today && "font-semibold text-accent",
                      date !== props.value &&
                        date !== today &&
                        !sameMonth(date, cursor()) &&
                        "text-muted",
                    )}
                    tabindex={date === cursor() ? 0 : -1}
                    aria-label={name(date)}
                    aria-pressed={date === props.value}
                    aria-current={date === today ? "date" : undefined}
                    disabled={!allowed(date)}
                    onClick={() => {
                      setCursor(date);
                      props.onChange(date);
                    }}
                  >
                    {Number(date.slice(8))}
                  </button>
                )}
              </For>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
