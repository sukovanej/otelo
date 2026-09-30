import { expect, test } from "vitest";

import type { FieldBody } from "@otelo/api";

import { typeLine, valuesTitle } from "../src/fieldHelp";

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

test("typeLine counts the records of the signal, or the resources", () => {
  expect(typeLine(route, "spans")).toBe(`string · ${(1200).toLocaleString()} spans`);
  expect(typeLine({ ...route, count: 1 }, "logs")).toBe("string · 1 log line");
  expect(typeLine({ ...route, count: 2 }, "metrics")).toBe("string · 2 series");
  expect(typeLine({ ...route, source: "resource", count: 3 }, "logs")).toBe("string · 3 resources");
  expect(typeLine({ ...route, source: "builtin", type: "duration", count: null }, "spans")).toBe(
    "duration",
  );
});

test("valuesTitle says whether the values are all of them", () => {
  expect(valuesTitle(route)).toBe("2 values");
  expect(valuesTitle({ ...route, values: [value("1")], distinct_values: 1 })).toBe("1 value");
  expect(valuesTitle({ ...route, distinct_values: 37 })).toBe("Most common of 37 values");
  expect(valuesTitle({ ...route, distinct_values: 200, many_values: true })).toBe(
    "Most common of 200+ values",
  );
  expect(valuesTitle({ ...route, many_values: true })).toBe("Most common of 2+ values");
});
