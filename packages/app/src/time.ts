export function parseTime(rfc3339: string): Date {
  // Date reads three fractional digits at most everywhere.
  return new Date(rfc3339.replace(/(\.\d{3})\d+/, "$1"));
}

export function formatTime(date: Date, now = new Date()): string {
  return isSameDay(date, now)
    ? formatClock(date)
    : `${padWithZeros(date.getMonth() + 1)}-${padWithZeros(date.getDate())} ${formatClock(date)}`;
}

export function formatAge(date: Date, now = new Date()): string {
  const seconds = Math.max(0, Math.round((now.getTime() - date.getTime()) / 1000));
  if (seconds < 1) return "now";
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86_400)}d ago`;
}

export function formatDateTime(date: Date): string {
  return `${date.getFullYear()}-${padWithZeros(date.getMonth() + 1)}-${padWithZeros(date.getDate())} ${formatClock(date)}`;
}

export function measureNanosBetween(start: string, time: string): number {
  const [startMillis, startNanos] = splitIntoMillisAndNanos(start);
  const [millis, nanos] = splitIntoMillisAndNanos(time);
  return (millis - startMillis) * 1e6 + nanos - startNanos;
}

// A number holds epoch nanoseconds to about a microsecond.
export function measureNanosSince(start: string, epochNanos: number): number {
  const [startMillis, startNanos] = splitIntoMillisAndNanos(start);
  return epochNanos - startMillis * 1e6 - startNanos;
}

function padWithZeros(value: number, width = 2): string {
  return String(value).padStart(width, "0");
}

function isSameDay(date: Date, other: Date): boolean {
  return (
    date.getFullYear() === other.getFullYear() &&
    date.getMonth() === other.getMonth() &&
    date.getDate() === other.getDate()
  );
}

function formatClock(date: Date): string {
  return `${padWithZeros(date.getHours())}:${padWithZeros(date.getMinutes())}:${padWithZeros(date.getSeconds())}.${padWithZeros(date.getMilliseconds(), 3)}`;
}

type MillisAndNanos = readonly [epochMillis: number, nanosPastMillis: number];

function splitIntoMillisAndNanos(rfc3339: string): MillisAndNanos {
  const fraction = /\.(\d+)/.exec(rfc3339)?.[1] ?? "";
  return [parseTime(rfc3339).getTime(), Number(fraction.padEnd(9, "0").slice(3, 9))];
}
