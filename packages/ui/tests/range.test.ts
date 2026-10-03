import { expect, test } from "vitest";

import {
  findPreset,
  formatApiDuration,
  formatHumanDuration,
  formatRangeLabel,
  moveRangeToNow,
  parseDuration,
  resizeRange,
  resolveRange,
  shiftRange,
  splitRangeLabel,
  UNTIL_NOW,
} from "../src/range";

const HOUR_MS = 3_600_000;

const SEPTEMBER_30_NOON_MS = onSeptember30(12).getTime();

test("parseDuration reads the units of the API", () => {
  expect(parseDuration("5m")).toBe(300_000);
  expect(parseDuration("1h")).toBe(HOUR_MS);
  expect(parseDuration("7d")).toBe(7 * 24 * HOUR_MS);
  expect(parseDuration("1w")).toBe(7 * 24 * HOUR_MS);
  expect(parseDuration("500ms")).toBe(500);
  expect(parseDuration("1h 30m")).toBe(1.5 * HOUR_MS);
  expect(parseDuration("1h30m")).toBe(1.5 * HOUR_MS);
});

test("parseDuration refuses what is no duration", () => {
  expect(parseDuration("")).toBeUndefined();
  expect(parseDuration("1")).toBeUndefined();
  expect(parseDuration("h")).toBeUndefined();
  expect(parseDuration("2026-09-30T10:00:00Z")).toBeUndefined();
});

test("formatApiDuration writes the longest unit that holds the duration whole", () => {
  expect(formatApiDuration(2 * 24 * HOUR_MS)).toBe("2d");
  expect(formatApiDuration(HOUR_MS)).toBe("1h");
  expect(formatApiDuration(1.5 * HOUR_MS)).toBe("90m");
  expect(formatApiDuration(807_000)).toBe("807s");
  expect(formatApiDuration(1500)).toBe("1500ms");
  expect(parseDuration(formatApiDuration(807_500))).toBe(807_500);
});

test("formatHumanDuration keeps the two longest units", () => {
  expect(formatHumanDuration(HOUR_MS)).toBe("1h");
  expect(formatHumanDuration(807_000)).toBe("13m 27s");
  expect(formatHumanDuration(26 * HOUR_MS + 61_000)).toBe("1d 2h");
  expect(formatHumanDuration(24 * HOUR_MS + 5 * 60_000)).toBe("1d 5m");
  expect(formatHumanDuration(250)).toBe("250ms");
});

test("resolveRange reads durations before now and timestamps", () => {
  expect(resolveRange({ since: "1h", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toEqual({
    startMs: SEPTEMBER_30_NOON_MS - HOUR_MS,
    endMs: SEPTEMBER_30_NOON_MS,
  });
  expect(resolveRange({ since: "2h", until: "1h" }, SEPTEMBER_30_NOON_MS)).toEqual({
    startMs: SEPTEMBER_30_NOON_MS - 2 * HOUR_MS,
    endMs: SEPTEMBER_30_NOON_MS - HOUR_MS,
  });
  expect(
    resolveRange(
      { since: toIso(onSeptember30(9)), until: toIso(onSeptember30(10)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toEqual({
    startMs: onSeptember30(9).getTime(),
    endMs: onSeptember30(10).getTime(),
  });
  expect(
    resolveRange(
      { since: "2026-09-30T10:00:00.123456789Z", until: "2026-09-30T10:00:01Z" },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toEqual({
    startMs: Date.UTC(2026, 8, 30, 10, 0, 0, 123),
    endMs: Date.UTC(2026, 8, 30, 10, 0, 1),
  });
});

test("resolveRange refuses a range it cannot read or that ends before it starts", () => {
  expect(resolveRange({ since: "soon", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toBeUndefined();
  expect(resolveRange({ since: "1", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toBeUndefined();
  expect(resolveRange({ since: "1h", until: "2h" }, SEPTEMBER_30_NOON_MS)).toBeUndefined();
  expect(
    resolveRange(
      { since: toIso(onSeptember30(10)), until: toIso(onSeptember30(10)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBeUndefined();
});

test("findPreset knows a preset however its duration is written", () => {
  expect(findPreset({ since: "1h", until: UNTIL_NOW })?.label).toBe("Last hour");
  expect(findPreset({ since: "24h", until: UNTIL_NOW })?.label).toBe("Last day");
  expect(findPreset({ since: "1w", until: UNTIL_NOW })?.label).toBe("Last week");
  expect(findPreset({ since: "15m", until: UNTIL_NOW })).toBeUndefined();
  expect(findPreset({ since: "1h", until: "30m" })).toBeUndefined();
  expect(findPreset({ since: "soon", until: UNTIL_NOW })).toBeUndefined();
});

test("shiftRange moves a range that ends now one length back", () => {
  expect(shiftRange({ since: "1h", until: UNTIL_NOW }, -1, SEPTEMBER_30_NOON_MS)).toEqual({
    since: toIso(onSeptember30(10)),
    until: toIso(onSeptember30(11)),
  });
});

test("shiftRange moves a range in the past one length either way", () => {
  const range = { since: toIso(onSeptember30(8)), until: toIso(onSeptember30(9)) };
  expect(shiftRange(range, -1, SEPTEMBER_30_NOON_MS)).toEqual({
    since: toIso(onSeptember30(7)),
    until: toIso(onSeptember30(8)),
  });
  expect(shiftRange(range, 1, SEPTEMBER_30_NOON_MS)).toEqual({
    since: toIso(onSeptember30(9)),
    until: toIso(onSeptember30(10)),
  });
});

test("shiftRange ends a range now that would end after now", () => {
  expect(
    shiftRange(
      { since: toIso(onSeptember30(10)), until: toIso(onSeptember30(11)) },
      1,
      SEPTEMBER_30_NOON_MS,
    ),
  ).toEqual({ since: "1h", until: UNTIL_NOW });
  expect(
    shiftRange(
      { since: toIso(onSeptember30(10, 30)), until: toIso(onSeptember30(11, 30)) },
      1,
      SEPTEMBER_30_NOON_MS,
    ),
  ).toEqual({ since: "1h", until: UNTIL_NOW });
  expect(shiftRange({ since: "1h", until: UNTIL_NOW }, 1, SEPTEMBER_30_NOON_MS)).toEqual({
    since: "1h",
    until: UNTIL_NOW,
  });
});

test("shiftRange leaves a range it cannot read", () => {
  expect(shiftRange({ since: "soon", until: UNTIL_NOW }, -1, SEPTEMBER_30_NOON_MS)).toBeUndefined();
});

test("moveRangeToNow keeps the length of the range", () => {
  expect(
    moveRangeToNow(
      { since: toIso(onSeptember30(8)), until: toIso(onSeptember30(9)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toEqual({ since: "1h", until: UNTIL_NOW });
  expect(
    moveRangeToNow(
      { since: toIso(onSeptember30(8)), until: toIso(onSeptember30(8, 13, 27)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toEqual({ since: "807s", until: UNTIL_NOW });
  expect(moveRangeToNow({ since: "soon", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toBeUndefined();
});

test("formatRangeLabel names a range that ends now by its length", () => {
  expect(formatRangeLabel({ since: "5m", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toBe(
    "Last 5 minutes",
  );
  expect(formatRangeLabel({ since: "24h", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toBe(
    "Last day",
  );
  expect(formatRangeLabel({ since: "807s", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toBe(
    "Last 13m 27s",
  );
});

test("formatRangeLabel names the days of a range in the past", () => {
  expect(
    formatRangeLabel(
      { since: toIso(onSeptember30(8)), until: toIso(onSeptember30(9, 30)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBe("Today 08:00 – 09:30");
  expect(
    formatRangeLabel(
      { since: toIso(onSeptemberDay(29, 8)), until: toIso(onSeptemberDay(29, 9)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBe("Yesterday 08:00 – 09:00");
  expect(
    formatRangeLabel(
      { since: toIso(onSeptemberDay(27, 8)), until: toIso(onSeptemberDay(27, 9)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBe("Sep 27 08:00 – 09:00");
  expect(
    formatRangeLabel(
      { since: toIso(onSeptemberDay(27, 8)), until: toIso(onSeptember30(9)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBe("Sep 27 08:00 – Today 09:00");
  expect(
    formatRangeLabel(
      { since: toIso(new Date(2025, 11, 31, 23)), until: toIso(new Date(2026, 0, 1, 1)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBe("Dec 31, 2025 23:00 – Jan 1 01:00");
});

test("formatRangeLabel shows seconds when either time has them", () => {
  expect(
    formatRangeLabel(
      { since: toIso(onSeptember30(8)), until: toIso(onSeptember30(9, 30, 5)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBe("Today 08:00:00 – 09:30:05");
  expect(
    formatRangeLabel(
      { since: toIso(onSeptember30(8, 0, 5)), until: toIso(onSeptember30(9)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBe("Today 08:00:05 – 09:00:00");
});

test("formatRangeLabel ends a range from a time with now", () => {
  expect(
    formatRangeLabel({ since: toIso(onSeptember30(8)), until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS),
  ).toBe("Today 08:00 – now");
  expect(
    formatRangeLabel(
      { since: toIso(onSeptemberDay(29, 8)), until: UNTIL_NOW },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toBe("Yesterday 08:00 – now");
});

test("splitRangeLabel dims the days and the dash", () => {
  expect(splitRangeLabel({ since: "1h", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toEqual([
    { text: "Last hour", dim: false },
  ]);
  expect(
    splitRangeLabel(
      { since: toIso(onSeptemberDay(27, 8)), until: toIso(onSeptember30(9)) },
      SEPTEMBER_30_NOON_MS,
    ),
  ).toEqual([
    { text: "Sep 27", dim: true },
    { text: "08:00", dim: false },
    { text: "–", dim: true },
    { text: "Today", dim: true },
    { text: "09:00", dim: false },
  ]);
});

test("resizeRange steps a range that ends now by minutes, hours, then days", () => {
  expect(lengthenRangeToNow("1h")).toBe("2h");
  expect(lengthenRangeToNow("2h")).toBe("3h");
  expect(lengthenRangeToNow("5m")).toBe("10m");
  expect(lengthenRangeToNow("55m")).toBe("1h");
  expect(lengthenRangeToNow("90m")).toBe("2h");
  expect(lengthenRangeToNow("23h")).toBe("1d");
  expect(lengthenRangeToNow("7d")).toBe("8d");
  expect(lengthenRangeToNow("2m")).toBe("5m");
});

test("resizeRange shortens a range that ends now down to five minutes", () => {
  expect(shortenRangeToNow("3h")).toBe("2h");
  expect(shortenRangeToNow("1h")).toBe("55m");
  expect(shortenRangeToNow("1d")).toBe("23h");
  expect(shortenRangeToNow("90m")).toBe("1h");
  expect(shortenRangeToNow("10m")).toBe("5m");
  expect(shortenRangeToNow("5m")).toBeUndefined();
  expect(shortenRangeToNow("soon")).toBeUndefined();
});

test("resizeRange keeps the end of a range in the past", () => {
  const range = { since: toIso(onSeptember30(8)), until: toIso(onSeptember30(9)) };
  expect(resizeRange(range, 1, SEPTEMBER_30_NOON_MS)).toEqual({
    since: toIso(onSeptember30(7)),
    until: toIso(onSeptember30(9)),
  });
  expect(resizeRange(range, -1, SEPTEMBER_30_NOON_MS)).toEqual({
    since: toIso(onSeptember30(8, 5)),
    until: toIso(onSeptember30(9)),
  });
});

test("formatRangeLabel shows a range it cannot read as it is", () => {
  expect(formatRangeLabel({ since: "soon", until: UNTIL_NOW }, SEPTEMBER_30_NOON_MS)).toBe("soon");
  expect(formatRangeLabel({ since: "soon", until: "later" }, SEPTEMBER_30_NOON_MS)).toBe(
    "soon – later",
  );
});

function lengthenRangeToNow(since: string): string | undefined {
  return resizeRange({ since, until: UNTIL_NOW }, 1, SEPTEMBER_30_NOON_MS)?.since;
}

function shortenRangeToNow(since: string): string | undefined {
  return resizeRange({ since, until: UNTIL_NOW }, -1, SEPTEMBER_30_NOON_MS)?.since;
}

function onSeptember30(hours: number, minutes = 0, seconds = 0): Date {
  return new Date(2026, 8, 30, hours, minutes, seconds);
}

function onSeptemberDay(dayOfMonth: number, hours: number): Date {
  return new Date(2026, 8, dayOfMonth, hours);
}

function toIso(date: Date): string {
  return date.toISOString();
}
