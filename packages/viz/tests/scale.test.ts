import { expect, test } from "vitest";

import { formatTick, niceTicks, timeTicks } from "../src/scale";

test("niceTicks reach past the largest value in round steps", () => {
  expect(niceTicks(0)).toEqual([0, 0.25, 0.5, 0.75, 1]);
  expect(niceTicks(7)).toEqual([0, 2, 4, 6, 8]);
  expect(niceTicks(100)).toEqual([0, 25, 50, 75, 100]);
  expect(niceTicks(0.05)).toEqual([0, 0.02, 0.04, 0.06]);
});

test("timeTicks fall on round local times", () => {
  const start = new Date(2026, 8, 28, 10, 7).getTime();
  const end = new Date(2026, 8, 28, 11, 7).getTime();
  expect(timeTicks(start, end, 6).map(formatTick)).toEqual([
    "10:10",
    "10:20",
    "10:30",
    "10:40",
    "10:50",
    "11:00",
  ]);
  expect(timeTicks(start, end, 4).map(formatTick)).toEqual(["10:15", "10:30", "10:45", "11:00"]);
});

test("formatTick shows the date at a midnight and seconds between minutes", () => {
  expect(formatTick(new Date(2026, 8, 29).getTime())).toBe("09-29");
  expect(formatTick(new Date(2026, 8, 29, 10, 0, 30).getTime())).toBe("10:00:30");
});
