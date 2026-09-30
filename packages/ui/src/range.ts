import { addDays, formatDate, formatMonthDay, toLocalClock, toLocalDate } from "./date";

export const UNTIL_NOW = "";

export const RANGE_PRESETS: ReadonlyArray<RangePreset> = [
  { since: "5m", label: "Last 5 minutes" },
  { since: "1h", label: "Last hour" },
  { since: "1d", label: "Last day" },
  { since: "7d", label: "Last week" },
];

const SECOND_MS = 1000;
const MINUTE_MS = 60 * SECOND_MS;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

const UNIT_MS: Record<string, number> = {
  ms: 1,
  s: SECOND_MS,
  m: MINUTE_MS,
  h: HOUR_MS,
  d: DAY_MS,
  w: 7 * DAY_MS,
};

const UNITS_LONGEST_FIRST = [
  ["d", DAY_MS],
  ["h", HOUR_MS],
  ["m", MINUTE_MS],
  ["s", SECOND_MS],
] as const;

const DURATION_PATTERN = /^(?:\s*\d+(?:\.\d+)?\s*(?:ms|s|m|h|d|w))+\s*$/;
const DURATION_PART_PATTERN = /(\d+(?:\.\d+)?)\s*(ms|s|m|h|d|w)/g;
const RFC_3339_PATTERN = /^\d{4}-\d{2}-\d{2}T/;

export interface Range {
  readonly since: string;
  readonly until: string;
}

interface ResolvedRange {
  readonly startMs: number;
  readonly endMs: number;
}

interface RangePreset {
  readonly since: string;
  readonly label: string;
}

interface RangeLabelPart {
  readonly text: string;
  readonly dim: boolean;
}

export function parseDuration(text: string): number | undefined {
  if (!DURATION_PATTERN.test(text)) return undefined;
  let totalMs = 0;
  for (const [, amount, unit = ""] of text.matchAll(DURATION_PART_PATTERN)) {
    totalMs += Number(amount) * (UNIT_MS[unit] ?? 0);
  }
  return totalMs;
}

export function formatApiDuration(durationMs: number): string {
  const wholeMs = Math.max(1, Math.round(durationMs));
  for (const [unit, unitMs] of UNITS_LONGEST_FIRST) {
    if (wholeMs % unitMs === 0) return `${wholeMs / unitMs}${unit}`;
  }
  return `${wholeMs}ms`;
}

export function formatHumanDuration(durationMs: number): string {
  if (durationMs < SECOND_MS) return `${Math.round(durationMs)}ms`;
  let restMs = Math.round(durationMs / SECOND_MS) * SECOND_MS;
  const parts: string[] = [];
  for (const [unit, unitMs] of UNITS_LONGEST_FIRST) {
    const count = Math.floor(restMs / unitMs);
    if (count > 0) parts.push(`${count}${unit}`);
    restMs -= count * unitMs;
  }
  return parts.slice(0, 2).join(" ");
}

export function resolveRange(range: Range, nowMs: number): ResolvedRange | undefined {
  const startMs = parseInstant(range.since, nowMs);
  const endMs = range.until === UNTIL_NOW ? nowMs : parseInstant(range.until, nowMs);
  if (startMs === undefined || endMs === undefined || startMs >= endMs) return undefined;
  return { startMs, endMs };
}

export function findPreset(range: Range): RangePreset | undefined {
  if (range.until !== UNTIL_NOW) return undefined;
  const lengthMs = parseDuration(range.since);
  return RANGE_PRESETS.find((preset) => parseDuration(preset.since) === lengthMs);
}

export function moveRangeToNow(range: Range, nowMs: number): Range | undefined {
  const resolved = resolveRange(range, nowMs);
  return (
    resolved && {
      since: formatApiDuration(resolved.endMs - resolved.startMs),
      until: UNTIL_NOW,
    }
  );
}

export function shiftRange(range: Range, step: 1 | -1, nowMs: number): Range | undefined {
  const resolved = resolveRange(range, nowMs);
  if (!resolved) return undefined;
  const lengthMs = resolved.endMs - resolved.startMs;
  const endMs = resolved.endMs + step * lengthMs;
  if (endMs >= nowMs) return { since: formatApiDuration(lengthMs), until: UNTIL_NOW };
  return {
    since: new Date(endMs - lengthMs).toISOString(),
    until: new Date(endMs).toISOString(),
  };
}

export function splitRangeLabel(range: Range, nowMs: number): RangeLabelPart[] {
  const preset = findPreset(range);
  if (preset) return [toPlainPart(preset.label)];
  const endsNow = range.until === UNTIL_NOW;
  const lengthMs = parseDuration(range.since);
  if (endsNow && lengthMs !== undefined) {
    return [toPlainPart(`Last ${formatHumanDuration(lengthMs)}`)];
  }
  const resolved = resolveRange(range, nowMs);
  if (!resolved) {
    return [toPlainPart(endsNow ? range.since : `${range.since} – ${range.until}`)];
  }
  const [start, end, today] = [
    new Date(resolved.startMs),
    new Date(resolved.endMs),
    new Date(nowMs),
  ];
  const showsSeconds = start.getSeconds() !== 0 || (!endsNow && end.getSeconds() !== 0);
  const toClockPart = (date: Date) =>
    toPlainPart(showsSeconds ? toLocalClock(date) : toLocalClock(date).slice(0, 5));
  const startParts = [toDimPart(formatDayName(start, today)), toClockPart(start), toDimPart("–")];
  if (endsNow) return [...startParts, toPlainPart("now")];
  if (isSameDay(start, end)) return [...startParts, toClockPart(end)];
  return [...startParts, toDimPart(formatDayName(end, today)), toClockPart(end)];
}

export function formatRangeLabel(range: Range, nowMs: number): string {
  return splitRangeLabel(range, nowMs)
    .map((part) => part.text)
    .join(" ");
}

function parseInstant(text: string, nowMs: number): number | undefined {
  const agoMs = parseDuration(text);
  if (agoMs !== undefined) return nowMs - agoMs;
  if (!RFC_3339_PATTERN.test(text)) return undefined;
  // Date reads three fractional digits at most everywhere.
  const instantMs = Date.parse(text.replace(/(\.\d{3})\d+/, "$1"));
  return Number.isNaN(instantMs) ? undefined : instantMs;
}

function isSameDay(date: Date, otherDate: Date): boolean {
  return toLocalDate(date) === toLocalDate(otherDate);
}

function toPlainPart(text: string): RangeLabelPart {
  return { text, dim: false };
}

function toDimPart(text: string): RangeLabelPart {
  return { text, dim: true };
}

function formatDayName(date: Date, now: Date): string {
  const [day, today] = [toLocalDate(date), toLocalDate(now)];
  if (day === today) return "Today";
  if (day === addDays(today, -1)) return "Yesterday";
  return date.getFullYear() === now.getFullYear() ? formatMonthDay(day) : formatDate(day);
}
