// The ticks of a chart's axes: round values from zero up, and round times on
// the local clock.

/** Round ticks from 0 up to at least `max`, `count` of them or a few more,
 * each 1, 2, 2.5, or 5 times a power of ten. A `max` of 0 has ticks up to 1. */
export function niceTicks(max: number, count = 4): number[] {
  const top = max > 0 && Number.isFinite(max) ? max : 1;
  const rough = top / count;
  const power = 10 ** Math.floor(Math.log10(rough));
  const step = ([1, 2, 2.5, 5, 10].find((m) => m * power >= rough) ?? 10) * power;
  const ticks = [0];
  for (let i = 1; (ticks.at(-1) ?? 0) < top; i++) ticks.push(Number((i * step).toPrecision(12)));
  return ticks;
}

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** The lengths of time an axis puts its ticks at, in milliseconds. */
const TIME_STEPS = [
  SECOND,
  5 * SECOND,
  10 * SECOND,
  15 * SECOND,
  30 * SECOND,
  MINUTE,
  5 * MINUTE,
  10 * MINUTE,
  15 * MINUTE,
  30 * MINUTE,
  HOUR,
  3 * HOUR,
  6 * HOUR,
  12 * HOUR,
  DAY,
];

/** Round times from `start` to `end`, in milliseconds, `most` of them at
 * most. Ticks of hours and days fall on the hours and the midnights of the
 * local clock. */
export function timeTicks(start: number, end: number, most: number): number[] {
  const span = end - start;
  const step = TIME_STEPS.find((s) => span / s <= Math.max(1, most)) ?? 2 * DAY;
  const offset = new Date(start).getTimezoneOffset() * MINUTE;
  const first = Math.ceil((start - offset) / step) * step + offset;
  const ticks: number[] = [];
  for (let t = first; t <= end; t += step) ticks.push(t);
  return ticks;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** `14:05`, `14:05:30` for a tick between minutes, or `09-28` at a
 * midnight. */
export function formatTick(ms: number): string {
  const date = new Date(ms);
  if (date.getHours() === 0 && date.getMinutes() === 0 && date.getSeconds() === 0) {
    return `${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  }
  const clock = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
  return date.getSeconds() === 0 ? clock : `${clock}:${pad(date.getSeconds())}`;
}

/** `14:05:00`, with the date before it when the time is not on the day of
 * `now`: `09-27 14:05:00`. */
export function formatInstant(ms: number, now = Date.now()): string {
  const date = new Date(ms);
  const today = new Date(now);
  const clock = `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  return date.toDateString() === today.toDateString()
    ? clock
    : `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${clock}`;
}
