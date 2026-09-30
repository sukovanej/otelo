// Dates and times of day as the pickers hold them, in local time: a date as
// `2026-09-30` and a time of day as `14:03:07`, which both sort as text.

const pad = (n: number, width = 2) => String(n).padStart(width, "0");

const DAY_SECONDS = 86_400;

/** The seconds in an hour, a minute, and a second: the parts of a time of
 * day, in their order. */
const PART_SECONDS = [3600, 60, 1] as const;

/** The hours, the minutes, or the seconds of a time of day. */
export type ClockPart = 0 | 1 | 2;

/** `2026-09-30`, the local date of `date`. */
export function dateOf(date: Date): string {
  return `${pad(date.getFullYear(), 4)}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** `14:03:07`, the local time of day of `date`. */
export function clockOf(date: Date): string {
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

const dateParts = (date: string): [number, number, number] => {
  const [year = 0, month = 1, day = 1] = date.split("-").map(Number);
  return [year, month, day];
};

const clockParts = (clock: string): [number, number, number] => {
  const [hours = 0, minutes = 0, seconds = 0] = clock.split(":").map(Number);
  return [hours, minutes, seconds];
};

/** The date of a year, a month from 1, and a day, either of which may be
 * past its ends: day 0 is the last day of the month before. */
const dateAt = (year: number, month: number, day: number) => dateOf(new Date(year, month - 1, day));

/** The local time of a date and a time of day. */
export function at(date: string, clock: string): Date {
  const [year, month, day] = dateParts(date);
  return new Date(year, month - 1, day, ...clockParts(clock));
}

export const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/** `Sep 30`, the month and the day of a date. */
export function monthDay(date: string): string {
  const [, month, day] = dateParts(date);
  return `${MONTHS[month - 1]?.slice(0, 3) ?? ""} ${day}`;
}

/** `Sep 30, 2026`, the date as a reader takes it in. */
export function formatDate(date: string): string {
  return `${monthDay(date)}, ${date.slice(0, 4)}`;
}

/** The month from 1 whose name starts with the three letters or more of
 * `name`, or 0 when no month does. */
const monthNamed = (name: string) =>
  name.length < 3
    ? 0
    : MONTHS.findIndex((month) => month.toLowerCase().startsWith(name.toLowerCase())) + 1;

/** The year, the month from 1, and the day that the text names. */
const typedDate = (text: string): [number, number, number] | undefined => {
  const numbers = /^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})$/.exec(text);
  if (numbers) return [Number(numbers[1]), Number(numbers[2]), Number(numbers[3])];
  const monthFirst = /^([a-z]+)\.? (\d{1,2}),? (\d{4})$/i.exec(text);
  if (monthFirst) {
    return [Number(monthFirst[3]), monthNamed(monthFirst[1] ?? ""), Number(monthFirst[2])];
  }
  const dayFirst = /^(\d{1,2})\.? ([a-z]+)\.?,? (\d{4})$/i.exec(text);
  if (dayFirst) return [Number(dayFirst[3]), monthNamed(dayFirst[2] ?? ""), Number(dayFirst[1])];
  return undefined;
};

/** The date typed as `Sep 30, 2026`, `30 September 2026`, `2026-09-30`,
 * `2026-9-30`, `2026/09/30`, or `2026.09.30`, or `undefined` when the text
 * is no date. */
export function parseDate(text: string): string | undefined {
  const typed = typedDate(text.trim().replace(/\s+/g, " "));
  if (!typed) return undefined;
  const [year, month, day] = typed;
  const date = `${pad(year, 4)}-${pad(month)}-${pad(day)}`;
  return dateAt(year, month, day) === date ? date : undefined;
}

/** The time of day typed as `14`, `14:03`, `14:03:07`, `1403`, or `140307`,
 * or `undefined` when the text is no time of day. */
export function parseClock(text: string): string | undefined {
  const typed = text.trim();
  const match =
    /^(\d{1,2})(?::(\d{1,2}))?(?::(\d{1,2}))?$/.exec(typed) ??
    /^(\d{2})(\d{2})(\d{2})?$/.exec(typed);
  if (!match) return undefined;
  const [hours, minutes, seconds] = [
    Number(match[1]),
    Number(match[2] ?? 0),
    Number(match[3] ?? 0),
  ];
  if (hours > 23 || minutes > 59 || seconds > 59) return undefined;
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
}

/** The part of a time of day that the caret at `caret` is in or next to. */
export function clockPart(caret: number): ClockPart {
  if (caret <= 2) return 0;
  return caret <= 5 ? 1 : 2;
}

/** The time of day `step` hours, minutes, or seconds later, around
 * midnight. */
export function stepClock(clock: string, part: ClockPart, step: number): string {
  const [hours, minutes, seconds] = clockParts(clock);
  const total = hours * 3600 + minutes * 60 + seconds + step * PART_SECONDS[part];
  const inDay = ((total % DAY_SECONDS) + DAY_SECONDS) % DAY_SECONDS;
  return `${pad(Math.floor(inDay / 3600))}:${pad(Math.floor(inDay / 60) % 60)}:${pad(inDay % 60)}`;
}

/** The date `days` later. */
export function addDays(date: string, days: number): string {
  const [year, month, day] = dateParts(date);
  return dateAt(year, month, day + days);
}

/** The date `months` later, on the last day of that month when it has no
 * such day: a month after January 31 is February 28. */
export function addMonths(date: string, months: number): string {
  const [year, month, day] = dateParts(date);
  const last = new Date(year, month + months, 0).getDate();
  return dateAt(year, month + months, Math.min(day, last));
}

/** How many days after Monday the date is. */
export function weekday(date: string): number {
  return (at(date, "00:00:00").getDay() + 6) % 7;
}

export const sameMonth = (a: string, b: string) => a.slice(0, 7) === b.slice(0, 7);

/** The six weeks, each from Monday, that hold the month of `date`. */
export function monthWeeks(date: string): string[][] {
  const [year, month] = dateParts(date);
  const lead = weekday(dateAt(year, month, 1));
  return Array.from({ length: 6 }, (_week, week) =>
    Array.from({ length: 7 }, (_day, day) => dateAt(year, month, 1 - lead + week * 7 + day)),
  );
}
