import { expect, test } from "vitest";

import { moveValue, pickDropIndex } from "../src/Select/reorder";

const TAGS = [
  { left: 0, right: 40, top: 0, bottom: 20 },
  { left: 44, right: 80, top: 0, bottom: 20 },
  { left: 0, right: 30, top: 24, bottom: 44 },
];

const ROWS = [
  { left: 0, right: 200, top: 0, bottom: 28 },
  { left: 0, right: 200, top: 28, bottom: 56 },
];

test("a dragged tag goes before the first tag it is left of, line by line", () => {
  expect(pickDropIndex("inline", TAGS, 10, 10)).toBe(0);
  expect(pickDropIndex("inline", TAGS, 50, 10)).toBe(1);
  expect(pickDropIndex("inline", TAGS, 90, 10)).toBe(2);
  expect(pickDropIndex("inline", TAGS, 5, 30)).toBe(2);
  expect(pickDropIndex("inline", TAGS, 40, 30)).toBe(3);
});

test("a dragged row goes after the rows whose middle is above the pointer", () => {
  expect(pickDropIndex("block", ROWS, 50, 5)).toBe(0);
  expect(pickDropIndex("block", ROWS, 50, 20)).toBe(1);
  expect(pickDropIndex("block", ROWS, 50, 60)).toBe(2);
});

test("a moved value takes its place among the others", () => {
  expect(moveValue(["a", "b", "c"], "a", 2)).toEqual(["b", "c", "a"]);
  expect(moveValue(["a", "b", "c"], "c", -1)).toEqual(["c", "a", "b"]);
});
