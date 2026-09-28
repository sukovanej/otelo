import { expect, test } from "vitest";

import {
  ago,
  formatDateTime,
  formatDuration,
  formatTime,
  nanosAfter,
  nanosBetween,
  parseTime,
} from "../src/time";

test("parseTime reads nanoseconds to the millisecond", () => {
  expect(parseTime("2026-09-28T10:00:00.123456789Z").toISOString()).toBe(
    "2026-09-28T10:00:00.123Z",
  );
  expect(parseTime("2026-09-28T10:00:00Z").toISOString()).toBe("2026-09-28T10:00:00.000Z");
});

test("formatTime shows the date of another day", () => {
  const now = new Date(2026, 8, 28, 15, 0, 0);
  expect(formatTime(new Date(2026, 8, 28, 14, 3, 7, 5), now)).toBe("14:03:07.005");
  expect(formatTime(new Date(2026, 8, 27, 14, 3, 7, 5), now)).toBe("09-27 14:03:07.005");
});

test("ago rounds down to the largest unit", () => {
  const now = new Date(2026, 8, 28, 15, 0, 0);
  const before = (ms: number) => new Date(now.getTime() - ms);
  expect(ago(now, now)).toBe("now");
  expect(ago(before(42_000), now)).toBe("42s ago");
  expect(ago(before(5 * 60_000 + 59_000), now)).toBe("5m ago");
  expect(ago(before(3 * 3_600_000), now)).toBe("3h ago");
  expect(ago(before(2 * 86_400_000), now)).toBe("2d ago");
});

test("formatDateTime always shows the date", () => {
  expect(formatDateTime(new Date(2026, 8, 28, 14, 3, 7, 5))).toBe("2026-09-28 14:03:07.005");
});

test("nanosBetween keeps the nanoseconds a Date drops", () => {
  const start = "2026-09-28T10:00:00.123456789Z";
  expect(nanosBetween(start, "2026-09-28T10:00:00.123456999Z")).toBe(210);
  expect(nanosBetween(start, "2026-09-28T10:00:01.5Z")).toBe(1_376_543_211);
  expect(nanosBetween(start, "2026-09-28T10:00:00Z")).toBe(-123_456_789);
});

test("nanosAfter measures to epoch nanoseconds", () => {
  const start = "2026-09-28T10:00:00.5Z";
  const epoch = Date.UTC(2026, 8, 28, 10, 0, 0) * 1e6;
  expect(nanosAfter(start, epoch + 750_000_000)).toBeCloseTo(250_000_000, -3);
});

test("formatDuration uses the units of the CLI", () => {
  expect(formatDuration(0)).toBe("0s");
  expect(formatDuration(850)).toBe("850ns");
  expect(formatDuration(12_345)).toBe("12.3µs");
  expect(formatDuration(4_560_000)).toBe("4.56ms");
  expect(formatDuration(1_200_000_000)).toBe("1.2s");
  expect(formatDuration(185e9)).toBe("3m05s");
  expect(formatDuration(120e9)).toBe("2m");
  expect(formatDuration(7800e9)).toBe("2h10m");
});

test("formatDuration moves a value that rounds to 1000 to the next unit", () => {
  expect(formatDuration(999_900)).toBe("1ms");
  expect(formatDuration(999_400)).toBe("999µs");
  expect(formatDuration(59_999_000_000)).toBe("1m");
});
