import { createEffect, createMemo, createSignal, For, on } from "solid-js";

import { ChevronIcon } from "@otelo/icons";

import { cx } from "../classes";
import {
  addDays,
  addMonths,
  countDaysAfterMonday,
  isSameMonth,
  listMonthWeeks,
  MONTH_NAMES,
  toLocalDate,
} from "../date";

const WEEKDAY_NAMES = [
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
  "Sunday",
];

const DATE_MOVES_BY_KEY: Record<string, (date: string) => string> = {
  ArrowLeft: (date) => addDays(date, -1),
  ArrowRight: (date) => addDays(date, 1),
  ArrowUp: (date) => addDays(date, -7),
  ArrowDown: (date) => addDays(date, 7),
  Home: (date) => addDays(date, -countDaysAfterMonday(date)),
  End: (date) => addDays(date, 6 - countDaysAfterMonday(date)),
  PageUp: (date) => addMonths(date, -1),
  PageDown: (date) => addMonths(date, 1),
};

const MONTH_BUTTON_CLASSES =
  "flex size-7 cursor-pointer items-center justify-center rounded-md hover:bg-hover";

interface CalendarProps {
  readonly value: string;
  readonly onChange: (date: string) => void;
  readonly label: string;
}

export default function Calendar(props: CalendarProps) {
  let dayGrid!: HTMLDivElement;
  const today = toLocalDate(new Date());
  const [focusedDate, setFocusedDate] = createSignal(props.value);
  createEffect(
    on(
      () => props.value,
      (value) => setFocusedDate(value),
      { defer: true },
    ),
  );

  const month = createMemo(() => focusedDate().slice(0, 7));
  const weeks = createMemo(() => listMonthWeeks(month()));
  const monthTitle = () =>
    `${MONTH_NAMES[Number(month().slice(5)) - 1] ?? ""} ${month().slice(0, 4)}`;

  const onKeyDown = (e: KeyboardEvent) => {
    const moveDate = DATE_MOVES_BY_KEY[e.key];
    if (!moveDate) return;
    e.preventDefault();
    setFocusedDate(moveDate(focusedDate()));
    dayGrid.querySelector<HTMLElement>("[tabindex='0']")?.focus();
  };

  return (
    <div class="w-max p-2 select-none" role="group" aria-label={props.label}>
      <div class="flex items-center justify-between pb-1">
        <button
          type="button"
          class={MONTH_BUTTON_CLASSES}
          aria-label="Previous month"
          onClick={() => setFocusedDate(addMonths(focusedDate(), -1))}
        >
          <ChevronIcon direction="left" size={13} />
        </button>
        <span class="font-medium" aria-live="polite">
          {monthTitle()}
        </span>
        <button
          type="button"
          class={MONTH_BUTTON_CLASSES}
          aria-label="Next month"
          onClick={() => setFocusedDate(addMonths(focusedDate(), 1))}
        >
          <ChevronIcon direction="right" size={13} />
        </button>
      </div>
      <div class="grid grid-cols-7 text-center text-2xs text-muted" aria-hidden="true">
        <For each={WEEKDAY_NAMES}>
          {(weekdayName) => <span class="py-1">{weekdayName.slice(0, 2)}</span>}
        </For>
      </div>
      <div ref={dayGrid} onKeyDown={onKeyDown}>
        <For each={weeks()}>
          {(week) => (
            <div class="grid grid-cols-7">
              <For each={week}>
                {(date) => (
                  <button
                    type="button"
                    class={cx(
                      "h-7 w-8 cursor-pointer rounded-md font-mono text-2xs",
                      date === props.value
                        ? "bg-accent font-semibold text-on-accent"
                        : "hover:bg-hover",
                      date !== props.value && date === today && "font-semibold text-accent",
                      date !== props.value &&
                        date !== today &&
                        !isSameMonth(date, focusedDate()) &&
                        "text-muted",
                    )}
                    tabindex={date === focusedDate() ? 0 : -1}
                    aria-label={formatDayLabel(date)}
                    aria-pressed={date === props.value}
                    aria-current={date === today ? "date" : undefined}
                    onClick={() => {
                      setFocusedDate(date);
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

function formatDayLabel(date: string): string {
  const weekdayName = WEEKDAY_NAMES[countDaysAfterMonday(date)] ?? "";
  const monthName = MONTH_NAMES[Number(date.slice(5, 7)) - 1] ?? "";
  return `${weekdayName} ${Number(date.slice(8))} ${monthName} ${date.slice(0, 4)}`;
}
