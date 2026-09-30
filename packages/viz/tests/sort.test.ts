import { expect, test } from "vitest";

import { sortRows } from "../src/sort";

const rows = [
  { name: "b", count: 2 },
  { name: "a", count: null },
  { name: "c", count: 2 },
  { name: "d", count: 9 },
];

test("rows sort by number either way, with missing values last", () => {
  expect(sortRows(rows, (row) => row.count, true).map((row) => row.name)).toEqual([
    "d",
    "b",
    "c",
    "a",
  ]);
  expect(sortRows(rows, (row) => row.count, false).map((row) => row.name)).toEqual([
    "b",
    "c",
    "d",
    "a",
  ]);
});

test("rows sort by text", () => {
  expect(sortRows(rows, (row) => row.name, false).map((row) => row.name)).toEqual([
    "a",
    "b",
    "c",
    "d",
  ]);
});
