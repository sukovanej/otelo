import { expect, test } from "vitest";

import { rate, seconds, share } from "../src/services/stats";

test("share and rate are null for an empty whole", () => {
  expect(share(1, 4)).toBe(0.25);
  expect(share(0, 0)).toBeNull();
  expect(rate(30, 60)).toBe(0.5);
  expect(rate(1, 0)).toBeNull();
});

test("seconds reads RFC 3339 times", () => {
  expect(seconds("2026-09-28T10:00:00Z", "2026-09-28T11:00:00.5Z")).toBe(3600.5);
});
