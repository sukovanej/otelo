import { expect, test } from "vitest";

import {
  formatAge,
  formatDateTime,
  formatTime,
  measureNanosBetween,
  measureNanosSince,
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

test("formatAge rounds down to the largest unit", () => {
  const now = new Date(2026, 8, 28, 15, 0, 0);
  const before = (ms: number) => new Date(now.getTime() - ms);
  expect(formatAge(now, now)).toBe("now");
  expect(formatAge(before(42_000), now)).toBe("42s ago");
  expect(formatAge(before(5 * 60_000 + 59_000), now)).toBe("5m ago");
  expect(formatAge(before(3 * 3_600_000), now)).toBe("3h ago");
  expect(formatAge(before(2 * 86_400_000), now)).toBe("2d ago");
});

test("formatDateTime always shows the date", () => {
  expect(formatDateTime(new Date(2026, 8, 28, 14, 3, 7, 5))).toBe("2026-09-28 14:03:07.005");
});

test("measureNanosBetween keeps the nanoseconds a Date drops", () => {
  const start = "2026-09-28T10:00:00.123456789Z";
  expect(measureNanosBetween(start, "2026-09-28T10:00:00.123456999Z")).toBe(210);
  expect(measureNanosBetween(start, "2026-09-28T10:00:01.5Z")).toBe(1_376_543_211);
  expect(measureNanosBetween(start, "2026-09-28T10:00:00Z")).toBe(-123_456_789);
});

test("measureNanosSince measures to epoch nanoseconds", () => {
  const start = "2026-09-28T10:00:00.5Z";
  const epoch = Date.UTC(2026, 8, 28, 10, 0, 0) * 1e6;
  expect(measureNanosSince(start, epoch + 750_000_000)).toBeCloseTo(250_000_000, -3);
});
