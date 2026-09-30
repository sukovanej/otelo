const SECOND_MS = 1000;
const MINUTE_MS = 60 * SECOND_MS;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

const TIME_TICK_STEPS_MS = [
  SECOND_MS,
  5 * SECOND_MS,
  10 * SECOND_MS,
  15 * SECOND_MS,
  30 * SECOND_MS,
  MINUTE_MS,
  5 * MINUTE_MS,
  10 * MINUTE_MS,
  15 * MINUTE_MS,
  30 * MINUTE_MS,
  HOUR_MS,
  3 * HOUR_MS,
  6 * HOUR_MS,
  12 * HOUR_MS,
  DAY_MS,
];

const TICK_STEP_MULTIPLIERS = [1, 2, 2.5, 5, 10];

export function pickValueTicks(max: number, count = 4): number[] {
  const top = max > 0 && Number.isFinite(max) ? max : 1;
  const rough = top / count;
  const power = 10 ** Math.floor(Math.log10(rough));
  const step =
    (TICK_STEP_MULTIPLIERS.find((multiplier) => multiplier * power >= rough) ?? 10) * power;
  const ticks = [0];
  for (let index = 1; (ticks.at(-1) ?? 0) < top; index++) {
    ticks.push(Number((index * step).toPrecision(12)));
  }
  return ticks;
}

export function pickTimeTicks(startMs: number, endMs: number, maxCount: number): number[] {
  const lengthMs = endMs - startMs;
  const tickStepMs =
    TIME_TICK_STEPS_MS.find((stepMs) => lengthMs / stepMs <= Math.max(1, maxCount)) ?? 2 * DAY_MS;
  // Ticks of hours and days fall on the hours and the midnights of the local clock.
  const offsetMs = new Date(startMs).getTimezoneOffset() * MINUTE_MS;
  const firstTickMs = Math.ceil((startMs - offsetMs) / tickStepMs) * tickStepMs + offsetMs;
  const ticks: number[] = [];
  for (let tickMs = firstTickMs; tickMs <= endMs; tickMs += tickStepMs) ticks.push(tickMs);
  return ticks;
}

export function formatTick(timeMs: number): string {
  const date = new Date(timeMs);
  if (date.getHours() === 0 && date.getMinutes() === 0 && date.getSeconds() === 0) {
    return `${padToTwoDigits(date.getMonth() + 1)}-${padToTwoDigits(date.getDate())}`;
  }
  const clock = `${padToTwoDigits(date.getHours())}:${padToTwoDigits(date.getMinutes())}`;
  return date.getSeconds() === 0 ? clock : `${clock}:${padToTwoDigits(date.getSeconds())}`;
}

export function formatInstant(timeMs: number, nowMs = Date.now()): string {
  const date = new Date(timeMs);
  const today = new Date(nowMs);
  const clock = `${padToTwoDigits(date.getHours())}:${padToTwoDigits(date.getMinutes())}:${padToTwoDigits(date.getSeconds())}`;
  return date.toDateString() === today.toDateString()
    ? clock
    : `${padToTwoDigits(date.getMonth() + 1)}-${padToTwoDigits(date.getDate())} ${clock}`;
}

function padToTwoDigits(number: number): string {
  return String(number).padStart(2, "0");
}
