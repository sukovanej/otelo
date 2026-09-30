import { expect, test } from "vitest";

import {
  formatDuration,
  humanDuration,
  parseDuration,
  presetOf,
  rangeLabel,
  rangeParts,
  resolve,
  shift,
  toNow,
} from "../src/range";

const HOUR = 3_600_000;

/** Noon of September 30 in local time, and times of that day. */
const day = (hours: number, minutes = 0, seconds = 0) =>
  new Date(2026, 8, 30, hours, minutes, seconds);
const now = day(12).getTime();
/** An hour of a day of that September. */
const on = (date: number, hours: number) => new Date(2026, 8, date, hours);
const iso = (date: Date) => date.toISOString();

test("parseDuration reads the units of the API", () => {
  expect(parseDuration("5m")).toBe(300_000);
  expect(parseDuration("1h")).toBe(HOUR);
  expect(parseDuration("7d")).toBe(7 * 24 * HOUR);
  expect(parseDuration("1w")).toBe(7 * 24 * HOUR);
  expect(parseDuration("500ms")).toBe(500);
  expect(parseDuration("1h 30m")).toBe(1.5 * HOUR);
  expect(parseDuration("1h30m")).toBe(1.5 * HOUR);
});

test("parseDuration refuses what is no duration", () => {
  expect(parseDuration("")).toBeUndefined();
  expect(parseDuration("1")).toBeUndefined();
  expect(parseDuration("h")).toBeUndefined();
  expect(parseDuration("2026-09-30T10:00:00Z")).toBeUndefined();
});

test("formatDuration writes the longest unit that holds the duration whole", () => {
  expect(formatDuration(2 * 24 * HOUR)).toBe("2d");
  expect(formatDuration(HOUR)).toBe("1h");
  expect(formatDuration(1.5 * HOUR)).toBe("90m");
  expect(formatDuration(807_000)).toBe("807s");
  expect(formatDuration(1500)).toBe("1500ms");
  expect(parseDuration(formatDuration(807_500))).toBe(807_500);
});

test("humanDuration keeps the two longest units", () => {
  expect(humanDuration(HOUR)).toBe("1h");
  expect(humanDuration(807_000)).toBe("13m 27s");
  expect(humanDuration(26 * HOUR + 61_000)).toBe("1d 2h");
  expect(humanDuration(24 * HOUR + 5 * 60_000)).toBe("1d 5m");
  expect(humanDuration(250)).toBe("250ms");
});

test("resolve reads durations before now and timestamps", () => {
  expect(resolve({ since: "1h", until: "" }, now)).toEqual({ start: now - HOUR, end: now });
  expect(resolve({ since: "2h", until: "1h" }, now)).toEqual({
    start: now - 2 * HOUR,
    end: now - HOUR,
  });
  expect(resolve({ since: iso(day(9)), until: iso(day(10)) }, now)).toEqual({
    start: day(9).getTime(),
    end: day(10).getTime(),
  });
  expect(
    resolve({ since: "2026-09-30T10:00:00.123456789Z", until: "2026-09-30T10:00:01Z" }, now),
  ).toEqual({ start: Date.UTC(2026, 8, 30, 10, 0, 0, 123), end: Date.UTC(2026, 8, 30, 10, 0, 1) });
});

test("resolve refuses a range it cannot read or that ends before it starts", () => {
  expect(resolve({ since: "soon", until: "" }, now)).toBeUndefined();
  expect(resolve({ since: "1", until: "" }, now)).toBeUndefined();
  expect(resolve({ since: "1h", until: "2h" }, now)).toBeUndefined();
  expect(resolve({ since: iso(day(10)), until: iso(day(10)) }, now)).toBeUndefined();
});

test("presetOf knows a preset however its duration is written", () => {
  expect(presetOf({ since: "1h", until: "" })?.label).toBe("Last hour");
  expect(presetOf({ since: "24h", until: "" })?.label).toBe("Last day");
  expect(presetOf({ since: "1w", until: "" })?.label).toBe("Last week");
  expect(presetOf({ since: "15m", until: "" })).toBeUndefined();
  expect(presetOf({ since: "1h", until: "30m" })).toBeUndefined();
  expect(presetOf({ since: "soon", until: "" })).toBeUndefined();
});

test("shift moves a range that ends now one length back", () => {
  expect(shift({ since: "1h", until: "" }, -1, now)).toEqual({
    since: iso(day(10)),
    until: iso(day(11)),
  });
});

test("shift moves a range in the past one length either way", () => {
  const range = { since: iso(day(8)), until: iso(day(9)) };
  expect(shift(range, -1, now)).toEqual({ since: iso(day(7)), until: iso(day(8)) });
  expect(shift(range, 1, now)).toEqual({ since: iso(day(9)), until: iso(day(10)) });
});

test("shift ends a range now that would end after now", () => {
  expect(shift({ since: iso(day(10)), until: iso(day(11)) }, 1, now)).toEqual({
    since: "1h",
    until: "",
  });
  expect(shift({ since: iso(day(10, 30)), until: iso(day(11, 30)) }, 1, now)).toEqual({
    since: "1h",
    until: "",
  });
  expect(shift({ since: "1h", until: "" }, 1, now)).toEqual({ since: "1h", until: "" });
});

test("shift leaves a range it cannot read", () => {
  expect(shift({ since: "soon", until: "" }, -1, now)).toBeUndefined();
});

test("toNow keeps the length of the range", () => {
  expect(toNow({ since: iso(day(8)), until: iso(day(9)) }, now)).toEqual({
    since: "1h",
    until: "",
  });
  expect(toNow({ since: iso(day(8)), until: iso(day(8, 13, 27)) }, now)).toEqual({
    since: "807s",
    until: "",
  });
  expect(toNow({ since: "soon", until: "" }, now)).toBeUndefined();
});

test("rangeLabel names a range that ends now by its length", () => {
  expect(rangeLabel({ since: "5m", until: "" }, now)).toBe("Last 5 minutes");
  expect(rangeLabel({ since: "24h", until: "" }, now)).toBe("Last day");
  expect(rangeLabel({ since: "807s", until: "" }, now)).toBe("Last 13m 27s");
});

test("rangeLabel names the days of a range in the past", () => {
  expect(rangeLabel({ since: iso(day(8)), until: iso(day(9, 30)) }, now)).toBe(
    "Today 08:00 – 09:30",
  );
  expect(rangeLabel({ since: iso(on(29, 8)), until: iso(on(29, 9)) }, now)).toBe(
    "Yesterday 08:00 – 09:00",
  );
  expect(rangeLabel({ since: iso(on(27, 8)), until: iso(on(27, 9)) }, now)).toBe(
    "Sep 27 08:00 – 09:00",
  );
  expect(rangeLabel({ since: iso(on(27, 8)), until: iso(day(9)) }, now)).toBe(
    "Sep 27 08:00 – Today 09:00",
  );
  expect(
    rangeLabel(
      { since: iso(new Date(2025, 11, 31, 23)), until: iso(new Date(2026, 0, 1, 1)) },
      now,
    ),
  ).toBe("Dec 31, 2025 23:00 – Jan 1 01:00");
});

test("rangeLabel shows seconds when either time has them", () => {
  expect(rangeLabel({ since: iso(day(8)), until: iso(day(9, 30, 5)) }, now)).toBe(
    "Today 08:00:00 – 09:30:05",
  );
  expect(rangeLabel({ since: iso(day(8, 0, 5)), until: iso(day(9)) }, now)).toBe(
    "Today 08:00:05 – 09:00:00",
  );
});

test("rangeLabel ends a range from a time with now", () => {
  expect(rangeLabel({ since: iso(day(8)), until: "" }, now)).toBe("Today 08:00 – now");
  expect(rangeLabel({ since: iso(new Date(2026, 8, 29, 8)), until: "" }, now)).toBe(
    "Yesterday 08:00 – now",
  );
});

test("rangeParts dims the days and the dash", () => {
  expect(rangeParts({ since: "1h", until: "" }, now)).toEqual([{ text: "Last hour", dim: false }]);
  expect(rangeParts({ since: iso(new Date(2026, 8, 27, 8)), until: iso(day(9)) }, now)).toEqual([
    { text: "Sep 27", dim: true },
    { text: "08:00", dim: false },
    { text: "–", dim: true },
    { text: "Today", dim: true },
    { text: "09:00", dim: false },
  ]);
});

test("rangeLabel shows a range it cannot read as it is", () => {
  expect(rangeLabel({ since: "soon", until: "" }, now)).toBe("soon");
  expect(rangeLabel({ since: "soon", until: "later" }, now)).toBe("soon – later");
});
