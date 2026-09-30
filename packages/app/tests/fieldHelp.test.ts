import { expect, test } from "vitest";

import type { FieldBody } from "@otelo/api";

import { formatTypeLine, formatValuesTitle } from "../src/fieldHelp";

const route: FieldBody = {
  name: "http.route",
  source: "attribute",
  count: 1200,
  type: "string",
  values: [makeValue('"/a"'), makeValue('"/b"')],
  distinct_values: 2,
  has_more_values_than_listed: false,
};

const duration: FieldBody = {
  name: "duration",
  source: "builtin",
  description: "How long the span took.",
  type: "duration",
  values: [],
  distinct_values: 0,
  has_more_values_than_listed: false,
};

test("formatTypeLine counts the records of the signal, or the resources", () => {
  expect(formatTypeLine(route, "spans")).toBe(`string · ${(1200).toLocaleString()} spans`);
  expect(formatTypeLine({ ...route, count: 1 }, "logs")).toBe("string · 1 log line");
  expect(formatTypeLine({ ...route, count: 2 }, "metrics")).toBe("string · 2 series");
  expect(formatTypeLine({ ...route, source: "resource", count: 3 }, "logs")).toBe(
    "string · 3 resources",
  );
  expect(formatTypeLine(duration, "spans")).toBe("duration");
});

test("formatValuesTitle says whether the values are all of them", () => {
  expect(formatValuesTitle(route)).toBe("2 values");
  expect(formatValuesTitle({ ...route, values: [makeValue("1")], distinct_values: 1 })).toBe(
    "1 value",
  );
  expect(formatValuesTitle({ ...route, distinct_values: 37 })).toBe("Most common of 37 values");
  expect(
    formatValuesTitle({ ...route, distinct_values: 200, has_more_values_than_listed: true }),
  ).toBe("Most common of 200+ values");
  expect(formatValuesTitle({ ...route, has_more_values_than_listed: true })).toBe(
    "Most common of 2+ values",
  );
});

function makeValue(text: string) {
  return { text, count: 1 };
}
