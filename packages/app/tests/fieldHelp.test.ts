import { expect, test } from "vitest";

import type { FieldBody } from "@otelo/api";

import { formatTypeLine, formatValuesTitle } from "../src/fieldHelp";

const value = (text: string) => ({ text, count: 1 });

const route: FieldBody = {
  name: "http.route",
  source: "attribute",
  type: "string",
  count: 1200,
  description: null,
  values: [value('"/a"'), value('"/b"')],
  distinct_values: 2,
  many_values: false,
};

test("formatTypeLine counts the records of the signal, or the resources", () => {
  expect(formatTypeLine(route, "spans")).toBe(`string · ${(1200).toLocaleString()} spans`);
  expect(formatTypeLine({ ...route, count: 1 }, "logs")).toBe("string · 1 log line");
  expect(formatTypeLine({ ...route, count: 2 }, "metrics")).toBe("string · 2 series");
  expect(formatTypeLine({ ...route, source: "resource", count: 3 }, "logs")).toBe(
    "string · 3 resources",
  );
  expect(
    formatTypeLine({ ...route, source: "builtin", type: "duration", count: null }, "spans"),
  ).toBe("duration");
});

test("formatValuesTitle says whether the values are all of them", () => {
  expect(formatValuesTitle(route)).toBe("2 values");
  expect(formatValuesTitle({ ...route, values: [value("1")], distinct_values: 1 })).toBe("1 value");
  expect(formatValuesTitle({ ...route, distinct_values: 37 })).toBe("Most common of 37 values");
  expect(formatValuesTitle({ ...route, distinct_values: 200, many_values: true })).toBe(
    "Most common of 200+ values",
  );
  expect(formatValuesTitle({ ...route, many_values: true })).toBe("Most common of 2+ values");
});
