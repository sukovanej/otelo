// Times as the API sends them (RFC 3339 with up to nine fractional digits)
// and as the UI shows them (local time).

/** The time of an RFC 3339 timestamp, to the millisecond. */
export function parseTime(rfc3339: string): Date {
  // Date reads three fractional digits at most everywhere.
  return new Date(rfc3339.replace(/(\.\d{3})\d+/, "$1"));
}

const pad = (n: number, width = 2) => String(n).padStart(width, "0");

const sameDay = (a: Date, b: Date) =>
  a.getFullYear() === b.getFullYear() &&
  a.getMonth() === b.getMonth() &&
  a.getDate() === b.getDate();

const clock = (date: Date) =>
  `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`;

/** `14:03:07.123` for today, `09-27 14:03:07.123` for another day. */
export function formatTime(date: Date, now = new Date()): string {
  return sameDay(date, now)
    ? clock(date)
    : `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${clock(date)}`;
}

/** `now`, `42s ago`, `5m ago`, `3h ago`, or `2d ago`. */
export function ago(date: Date, now = new Date()): string {
  const seconds = Math.max(0, Math.round((now.getTime() - date.getTime()) / 1000));
  if (seconds < 1) return "now";
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86_400)}d ago`;
}

/** `2026-09-28 14:03:07.123` in local time. */
export function formatDateTime(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${clock(date)}`;
}
