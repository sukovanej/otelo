import { expect, test } from "vitest";

import {
  addDays,
  addMonths,
  countDaysAfterMonday,
  findClockPartAtCaret,
  formatDate,
  formatMonthDay,
  joinDateAndClock,
  listMonthWeeks,
  parseClock,
  parseDate,
  stepClock,
  toLocalClock,
  toLocalDate,
} from "../src/date";

test("toLocalDate and toLocalClock read the local date and time of day", () => {
  const date = new Date(2026, 8, 30, 14, 3, 7);
  expect(toLocalDate(date)).toBe("2026-09-30");
  expect(toLocalClock(date)).toBe("14:03:07");
});

test("joinDateAndClock joins a date and a time of day", () => {
  expect(joinDateAndClock("2026-09-30", "14:03:07")).toEqual(new Date(2026, 8, 30, 14, 3, 7));
});

test("parseDate reads a date with its usual separators", () => {
  expect(parseDate("2026-09-30")).toBe("2026-09-30");
  expect(parseDate(" 2026-9-3 ")).toBe("2026-09-03");
  expect(parseDate("2026/09/30")).toBe("2026-09-30");
  expect(parseDate("2026.09.30")).toBe("2026-09-30");
});

test("parseDate reads a date with the name of its month", () => {
  expect(parseDate("Sep 30, 2026")).toBe("2026-09-30");
  expect(parseDate("sep 3 2026")).toBe("2026-09-03");
  expect(parseDate("September  30,  2026")).toBe("2026-09-30");
  expect(parseDate("30 Sep 2026")).toBe("2026-09-30");
  expect(parseDate("30. September 2026")).toBe("2026-09-30");
  expect(parseDate("Sep 31, 2026")).toBeUndefined();
  expect(parseDate("Se 30, 2026")).toBeUndefined();
  expect(parseDate("Sept 30, 2026")).toBe("2026-09-30");
  expect(parseDate("Sepx 30, 2026")).toBeUndefined();
});

test("formatDate names the month, and parseDate reads it back", () => {
  expect(formatMonthDay("2026-09-03")).toBe("Sep 3");
  expect(formatDate("2026-09-03")).toBe("Sep 3, 2026");
  expect(parseDate(formatDate("2026-12-31"))).toBe("2026-12-31");
});

test("parseDate refuses what is no date", () => {
  expect(parseDate("2026-02-30")).toBeUndefined();
  expect(parseDate("2026-13-01")).toBeUndefined();
  expect(parseDate("26-09-30")).toBeUndefined();
  expect(parseDate("yesterday")).toBeUndefined();
  expect(parseDate("")).toBeUndefined();
});

test("parseClock fills the parts that are left out", () => {
  expect(parseClock("14:03:07")).toBe("14:03:07");
  expect(parseClock("14:03")).toBe("14:03:00");
  expect(parseClock("9")).toBe("09:00:00");
  expect(parseClock("9:5")).toBe("09:05:00");
  expect(parseClock("1403")).toBe("14:03:00");
  expect(parseClock("140307")).toBe("14:03:07");
});

test("parseClock refuses what is no time of day", () => {
  expect(parseClock("24:00")).toBeUndefined();
  expect(parseClock("12:60")).toBeUndefined();
  expect(parseClock("12:00:60")).toBeUndefined();
  expect(parseClock("noon")).toBeUndefined();
  expect(parseClock("")).toBeUndefined();
});

test("findClockPartAtCaret finds the part around the caret", () => {
  expect([0, 1, 2].map(findClockPartAtCaret)).toEqual(["hours", "hours", "hours"]);
  expect([3, 4, 5].map(findClockPartAtCaret)).toEqual(["minutes", "minutes", "minutes"]);
  expect([6, 7, 8].map(findClockPartAtCaret)).toEqual(["seconds", "seconds", "seconds"]);
});

test("stepClock steps one part and carries into the others", () => {
  expect(stepClock("14:03:07", "hours", 1)).toBe("15:03:07");
  expect(stepClock("14:03:07", "minutes", -1)).toBe("14:02:07");
  expect(stepClock("14:59:59", "seconds", 1)).toBe("15:00:00");
});

test("stepClock goes around midnight", () => {
  expect(stepClock("23:30:00", "hours", 1)).toBe("00:30:00");
  expect(stepClock("00:00:00", "seconds", -1)).toBe("23:59:59");
});

test("addDays crosses months and years", () => {
  expect(addDays("2026-09-30", 1)).toBe("2026-10-01");
  expect(addDays("2026-01-01", -1)).toBe("2025-12-31");
  expect(addDays("2026-09-30", 7)).toBe("2026-10-07");
});

test("addMonths stays in a month that is shorter", () => {
  expect(addMonths("2026-09-30", 1)).toBe("2026-10-30");
  expect(addMonths("2026-01-31", 1)).toBe("2026-02-28");
  expect(addMonths("2026-03-31", -1)).toBe("2026-02-28");
  expect(addMonths("2026-12-15", 1)).toBe("2027-01-15");
  expect(addMonths("2026-01-15", -1)).toBe("2025-12-15");
});

test("countDaysAfterMonday counts from Monday", () => {
  expect(countDaysAfterMonday("2026-09-28")).toBe(0);
  expect(countDaysAfterMonday("2026-09-30")).toBe(2);
  expect(countDaysAfterMonday("2026-10-04")).toBe(6);
});

test("listMonthWeeks holds the month in six weeks from Monday", () => {
  const weeks = listMonthWeeks("2026-09-30");
  expect(weeks).toHaveLength(6);
  expect(weeks[0]).toEqual([
    "2026-08-31",
    "2026-09-01",
    "2026-09-02",
    "2026-09-03",
    "2026-09-04",
    "2026-09-05",
    "2026-09-06",
  ]);
  expect(weeks[5]?.[6]).toBe("2026-10-11");
  expect(listMonthWeeks("2026-09")).toEqual(weeks);
});

test("listMonthWeeks starts a month that begins on Monday with its first day", () => {
  expect(listMonthWeeks("2026-06-10")[0]?.[0]).toBe("2026-06-01");
});
