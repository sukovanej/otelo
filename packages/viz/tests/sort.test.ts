import { expect, test } from "vitest";

import { sortRows } from "../src/sort";

const rows = [
  { name: "b", n: 2 },
  { name: "a", n: null },
  { name: "c", n: 2 },
  { name: "d", n: 9 },
];

test("rows sort by number either way, with missing values last", () => {
  expect(sortRows(rows, (r) => r.n, true).map((r) => r.name)).toEqual(["d", "b", "c", "a"]);
  expect(sortRows(rows, (r) => r.n, false).map((r) => r.name)).toEqual(["b", "c", "d", "a"]);
});

test("rows sort by text", () => {
  expect(sortRows(rows, (r) => r.name, false).map((r) => r.name)).toEqual(["a", "b", "c", "d"]);
});
