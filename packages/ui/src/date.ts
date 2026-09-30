export const MONTH_NAMES = [
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

export const CLOCK_PART_STARTS: Record<ClockPart, number> = { hours: 0, minutes: 3, seconds: 6 };

const DAY_SECONDS = 86_400;

const CLOCK_PART_SECONDS: Record<ClockPart, number> = { hours: 3600, minutes: 60, seconds: 1 };

type ClockPart = "hours" | "minutes" | "seconds";

export function toLocalDate(date: Date): string {
  return `${zeroPad(date.getFullYear(), 4)}-${zeroPad(date.getMonth() + 1)}-${zeroPad(date.getDate())}`;
}

export function toLocalClock(date: Date): string {
  return `${zeroPad(date.getHours())}:${zeroPad(date.getMinutes())}:${zeroPad(date.getSeconds())}`;
}

export function joinDateAndClock(date: string, clock: string): Date {
  const [year, month, day] = splitDate(date);
  return new Date(year, month - 1, day, ...splitClock(clock));
}

export function formatMonthDay(date: string): string {
  const [, month, day] = splitDate(date);
  return `${MONTH_NAMES[month - 1]?.slice(0, 3) ?? ""} ${day}`;
}

export function formatDate(date: string): string {
  return `${formatMonthDay(date)}, ${date.slice(0, 4)}`;
}

export function parseDate(text: string): string | undefined {
  const typed = readTypedDate(text.trim().replace(/\s+/g, " "));
  if (!typed) return undefined;
  const [year, month, day] = typed;
  const date = `${zeroPad(year, 4)}-${zeroPad(month)}-${zeroPad(day)}`;
  return normalizeDate(year, month, day) === date ? date : undefined;
}

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
  return `${zeroPad(hours)}:${zeroPad(minutes)}:${zeroPad(seconds)}`;
}

export function findClockPartAtCaret(caret: number): ClockPart {
  if (caret <= 2) return "hours";
  return caret <= 5 ? "minutes" : "seconds";
}

export function stepClock(clock: string, part: ClockPart, step: number): string {
  const [hours, minutes, seconds] = splitClock(clock);
  const totalSeconds = hours * 3600 + minutes * 60 + seconds + step * CLOCK_PART_SECONDS[part];
  const secondsIntoDay = ((totalSeconds % DAY_SECONDS) + DAY_SECONDS) % DAY_SECONDS;
  return `${zeroPad(Math.floor(secondsIntoDay / 3600))}:${zeroPad(Math.floor(secondsIntoDay / 60) % 60)}:${zeroPad(secondsIntoDay % 60)}`;
}

export function addDays(date: string, days: number): string {
  const [year, month, day] = splitDate(date);
  return normalizeDate(year, month, day + days);
}

export function addMonths(date: string, months: number): string {
  const [year, month, day] = splitDate(date);
  const lastDayOfMonth = new Date(year, month + months, 0).getDate();
  return normalizeDate(year, month + months, Math.min(day, lastDayOfMonth));
}

export function countDaysAfterMonday(date: string): number {
  return (joinDateAndClock(date, "00:00:00").getDay() + 6) % 7;
}

export function isSameMonth(date: string, otherDate: string): boolean {
  return date.slice(0, 7) === otherDate.slice(0, 7);
}

export function listMonthWeeks(date: string): string[][] {
  const [year, month] = splitDate(date);
  const daysBeforeMonth = countDaysAfterMonday(normalizeDate(year, month, 1));
  return Array.from({ length: 6 }, (_week, weekIndex) =>
    Array.from({ length: 7 }, (_day, dayIndex) =>
      normalizeDate(year, month, 1 - daysBeforeMonth + weekIndex * 7 + dayIndex),
    ),
  );
}

function zeroPad(value: number, width = 2): string {
  return String(value).padStart(width, "0");
}

function splitDate(date: string): [number, number, number] {
  const [year = 0, month = 1, day = 1] = date.split("-").map(Number);
  return [year, month, day];
}

function splitClock(clock: string): [number, number, number] {
  const [hours = 0, minutes = 0, seconds = 0] = clock.split(":").map(Number);
  return [hours, minutes, seconds];
}

function normalizeDate(year: number, month: number, day: number): string {
  return toLocalDate(new Date(year, month - 1, day));
}

function findMonthByName(name: string): number | undefined {
  if (name.length < 3) return undefined;
  const monthIndex = MONTH_NAMES.findIndex((monthName) =>
    monthName.toLowerCase().startsWith(name.toLowerCase()),
  );
  return monthIndex === -1 ? undefined : monthIndex + 1;
}

function readTypedDate(text: string): [number, number, number] | undefined {
  const numbers = /^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})$/.exec(text);
  if (numbers) return [Number(numbers[1]), Number(numbers[2]), Number(numbers[3])];
  const monthFirst = /^([a-z]+)\.? (\d{1,2}),? (\d{4})$/i.exec(text);
  if (monthFirst) {
    const month = findMonthByName(monthFirst[1] ?? "");
    return month === undefined ? undefined : [Number(monthFirst[3]), month, Number(monthFirst[2])];
  }
  const dayFirst = /^(\d{1,2})\.? ([a-z]+)\.?,? (\d{4})$/i.exec(text);
  if (dayFirst) {
    const month = findMonthByName(dayFirst[2] ?? "");
    return month === undefined ? undefined : [Number(dayFirst[3]), month, Number(dayFirst[1])];
  }
  return undefined;
}
