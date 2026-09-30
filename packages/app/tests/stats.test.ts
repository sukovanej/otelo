import { expect, test } from "vitest";

import { measureSeconds, toRate, toShare } from "../src/services/stats";

test("toShare and toRate are null for an empty whole", () => {
  expect(toShare(1, 4)).toBe(0.25);
  expect(toShare(0, 0)).toBeNull();
  expect(toRate(30, 60)).toBe(0.5);
  expect(toRate(1, 0)).toBeNull();
});

test("measureSeconds reads RFC 3339 times", () => {
  expect(measureSeconds("2026-09-28T10:00:00Z", "2026-09-28T11:00:00.5Z")).toBe(3600.5);
});
