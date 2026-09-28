import { expect, test } from "vitest";

import { formatDuration, formatValue, valueParts } from "../src/units";

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

test("a count compacts from ten thousand up", () => {
  expect(formatValue(0, "count")).toBe("0");
  expect(formatValue(1284, "count")).toBe("1,284");
  expect(formatValue(12_900, "count")).toBe("12.9K");
  expect(formatValue(999_499, "count")).toBe("999K");
  expect(formatValue(999_500, "count")).toBe("1M");
  expect(formatValue(4_200_000, "count")).toBe("4.2M");
  expect(formatValue(2.5, "count")).toBe("2.5");
});

test("a rate never rounds a rate above zero to zero", () => {
  expect(formatValue(0, "rate")).toBe("0/s");
  expect(formatValue(1 / 3600, "rate")).toBe("<0.01/s");
  expect(formatValue(0.05, "rate")).toBe("0.05/s");
  expect(formatValue(1.23, "rate")).toBe("1.23/s");
  expect(formatValue(25_000, "rate")).toBe("25K/s");
});

test("a ratio never shows a small share as 0% or a large one as 100%", () => {
  expect(formatValue(0, "ratio")).toBe("0%");
  expect(formatValue(0.000_01, "ratio")).toBe("<0.01%");
  expect(formatValue(0.001_234, "ratio")).toBe("0.12%");
  expect(formatValue(0.034, "ratio")).toBe("3.4%");
  expect(formatValue(0.9999, "ratio")).toBe(">99.9%");
  expect(formatValue(1, "ratio")).toBe("100%");
});

test("bytes go up by 1024", () => {
  expect(formatValue(512, "bytes")).toBe("512B");
  expect(formatValue(1536, "bytes")).toBe("1.5KiB");
  expect(formatValue(3.2 * 1024 ** 3, "bytes")).toBe("3.2GiB");
});

test("a value splits into its numbers and units", () => {
  expect(valueParts(185e9, "duration")).toEqual([
    { value: "3", unit: "m" },
    { value: "05", unit: "s" },
  ]);
  expect(valueParts(0.0267, "ratio")).toEqual([{ value: "2.67", unit: "%", tight: true }]);
});

test("a missing or endless value is a dash", () => {
  expect(formatValue(null, "count")).toBe("–");
  expect(formatValue(undefined, "duration")).toBe("–");
  expect(formatValue(Number.NaN, "number")).toBe("–");
});
