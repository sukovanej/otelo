// The range of a query as the API takes it: `since` and `until`, each a
// duration before now such as `2h` or an RFC 3339 timestamp, and an empty
// `until` for now.

import { addDays, clockOf, dateOf, formatDate, monthDay } from "./date";

export interface Range {
  since: string;
  until: string;
}

/** A range in milliseconds since the epoch. */
export interface Span {
  start: number;
  end: number;
}

/** A range that ends now, and what the picker calls it. */
export interface Preset {
  since: string;
  label: string;
}

export const PRESETS: readonly Preset[] = [
  { since: "5m", label: "Last 5 minutes" },
  { since: "1h", label: "Last hour" },
  { since: "1d", label: "Last day" },
  { since: "7d", label: "Last week" },
];

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const UNITS: Record<string, number> = {
  ms: 1,
  s: SECOND,
  m: MINUTE,
  h: HOUR,
  d: DAY,
  w: 7 * DAY,
};

/** The units a duration is written in, from the longest. */
const WRITTEN = [
  ["d", DAY],
  ["h", HOUR],
  ["m", MINUTE],
  ["s", SECOND],
] as const;

const DURATION = /^(?:\s*\d+(?:\.\d+)?\s*(?:ms|s|m|h|d|w))+\s*$/;
const DURATION_PART = /(\d+(?:\.\d+)?)\s*(ms|s|m|h|d|w)/g;

/** The milliseconds of a duration such as `90s`, `2h`, or `1h 30m`, or
 * `undefined` when the text is no duration. */
export function parseDuration(text: string): number | undefined {
  if (!DURATION.test(text)) return undefined;
  let total = 0;
  for (const [, amount, unit = ""] of text.matchAll(DURATION_PART)) {
    total += Number(amount) * (UNITS[unit] ?? 0);
  }
  return total;
}

/** The duration as the API reads it, in the longest unit that holds it
 * whole: `2h`, `90s`. */
export function formatDuration(ms: number): string {
  const whole = Math.max(1, Math.round(ms));
  for (const [unit, size] of WRITTEN) {
    if (whole % size === 0) return `${whole / size}${unit}`;
  }
  return `${whole}ms`;
}

/** The duration as a reader takes it in, to its two longest units: `1h`,
 * `13m 27s`, `2d 4h`. */
export function humanDuration(ms: number): string {
  if (ms < SECOND) return `${Math.round(ms)}ms`;
  let rest = Math.round(ms / SECOND) * SECOND;
  const parts: string[] = [];
  for (const [unit, size] of WRITTEN) {
    const count = Math.floor(rest / size);
    if (count > 0) parts.push(`${count}${unit}`);
    rest -= count * size;
  }
  return parts.slice(0, 2).join(" ");
}

const RFC_3339 = /^\d{4}-\d{2}-\d{2}T/;

const instant = (text: string, now: number): number | undefined => {
  const ago = parseDuration(text);
  if (ago !== undefined) return now - ago;
  if (!RFC_3339.test(text)) return undefined;
  // Date reads three fractional digits at most everywhere.
  const time = Date.parse(text.replace(/(\.\d{3})\d+/, "$1"));
  return Number.isNaN(time) ? undefined : time;
};

/** The start and the end of the range at `now`, or `undefined` when either
 * does not read as a time or the start is not before the end. */
export function resolve(range: Range, now: number): Span | undefined {
  const start = instant(range.since, now);
  const end = range.until === "" ? now : instant(range.until, now);
  if (start === undefined || end === undefined || start >= end) return undefined;
  return { start, end };
}

/** The preset the range is, however its duration is written: `24h` is the
 * last day. */
export function presetOf(range: Range): Preset | undefined {
  if (range.until !== "") return undefined;
  const length = parseDuration(range.since);
  return PRESETS.find((preset) => parseDuration(preset.since) === length);
}

/** The range of the same length that ends now. */
export function toNow(range: Range, now: number): Range | undefined {
  const span = resolve(range, now);
  return span && { since: formatDuration(span.end - span.start), until: "" };
}

/**
 * The range one length of itself later for a `step` of 1, or earlier for -1:
 * the last hour goes to the hour before it. A range that would end after
 * now ends now instead.
 */
export function shift(range: Range, step: 1 | -1, now: number): Range | undefined {
  const span = resolve(range, now);
  if (!span) return undefined;
  const length = span.end - span.start;
  const end = span.end + step * length;
  if (end >= now) return { since: formatDuration(length), until: "" };
  return { since: new Date(end - length).toISOString(), until: new Date(end).toISOString() };
}

const sameDay = (a: Date, b: Date) => dateOf(a) === dateOf(b);

/** A piece of the name of a range. The days and the dash between the ends
 * are `dim`, so the times of day stand out. */
export interface LabelPart {
  text: string;
  dim: boolean;
}

const plain = (text: string): LabelPart => ({ text, dim: false });
const dim = (text: string): LabelPart => ({ text, dim: true });

/** `Today`, `Yesterday`, `Sep 27`, or `Sep 27, 2025` in another year than
 * that of `now`. */
const dayName = (date: Date, now: Date): string => {
  const [day, today] = [dateOf(date), dateOf(now)];
  if (day === today) return "Today";
  if (day === addDays(today, -1)) return "Yesterday";
  return date.getFullYear() === now.getFullYear() ? monthDay(day) : formatDate(day);
};

/**
 * What the picker calls the range, in pieces: `Last hour`, `Last 13m 27s`,
 * `Today 14:05 – 15:05`, `Sep 27 14:05 – Sep 28 15:05`, or
 * `Yesterday 14:05 – now`. The times have seconds when either is not on a
 * whole minute.
 */
export function rangeParts(range: Range, now: number): LabelPart[] {
  const preset = presetOf(range);
  if (preset) return [plain(preset.label)];
  const endsNow = range.until === "";
  const length = parseDuration(range.since);
  if (endsNow && length !== undefined) return [plain(`Last ${humanDuration(length)}`)];
  const span = resolve(range, now);
  if (!span) return [plain(endsNow ? range.since : `${range.since} – ${range.until}`)];
  const [start, end, today] = [new Date(span.start), new Date(span.end), new Date(now)];
  const seconds = start.getSeconds() !== 0 || (!endsNow && end.getSeconds() !== 0);
  const time = (date: Date) => plain(seconds ? clockOf(date) : clockOf(date).slice(0, 5));
  const from = [dim(dayName(start, today)), time(start), dim("–")];
  if (endsNow) return [...from, plain("now")];
  if (sameDay(start, end)) return [...from, time(end)];
  return [...from, dim(dayName(end, today)), time(end)];
}

/** The name of the range in one line, for a reader that cannot see it. */
export function rangeLabel(range: Range, now: number): string {
  return rangeParts(range, now)
    .map((part) => part.text)
    .join(" ");
}
