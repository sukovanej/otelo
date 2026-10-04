import { expect, test } from "vitest";

import type { WidgetLayout } from "@otelo/api";

import {
  arrangeLayouts,
  pickGridColumnCount,
  resizeLayout,
  settleLayouts,
} from "../src/dashboards/layout";

function area(column: number, row: number, width: number, height: number): WidgetLayout {
  return { column, row, width, height };
}

test("a dropped widget keeps its place and pushes the ones under it down", () => {
  const layouts = [area(0, 0, 6, 4), area(6, 0, 6, 4), area(0, 4, 12, 4), area(3, 2, 6, 4)];
  expect(settleLayouts(layouts, 3)).toEqual([
    area(0, 4, 6, 4),
    area(6, 4, 6, 4),
    area(0, 8, 12, 4),
    area(3, 0, 6, 4),
  ]);
  expect(settleLayouts([area(0, 0, 6, 4), area(6, 0, 6, 4)], undefined)).toEqual([
    area(0, 0, 6, 4),
    area(6, 0, 6, 4),
  ]);
});

test("a widget dropped on the one below swaps with it, and back, without a gap", () => {
  const movedDown = settleLayouts([area(0, 6, 12, 6), area(0, 6, 12, 6)], 0);
  expect(movedDown).toEqual([area(0, 6, 12, 6), area(0, 0, 12, 6)]);
  expect(settleLayouts([area(0, 0, 12, 6), area(0, 0, 12, 6)], 0)).toEqual([
    area(0, 0, 12, 6),
    area(0, 6, 12, 6),
  ]);
});

test("the widgets float up into the space a moved or removed widget leaves", () => {
  const tiles = [area(0, 0, 3, 4), area(3, 0, 3, 4), area(0, 4, 8, 6), area(8, 4, 4, 6)];
  const pushed = settleLayouts([area(2, 4, 3, 4), ...tiles.slice(1)], 0);
  expect(pushed).toEqual([area(2, 4, 3, 4), area(3, 0, 3, 4), area(0, 8, 8, 6), area(8, 0, 4, 6)]);
  expect(settleLayouts([area(0, 0, 3, 4), ...pushed.slice(1)], 0)).toEqual([
    area(0, 0, 3, 4),
    area(3, 0, 3, 4),
    area(0, 4, 8, 6),
    area(8, 0, 4, 6),
  ]);
  expect(
    settleLayouts([area(0, 0, 6, 4), area(6, 9, 6, 4), area(0, 20, 12, 2)], undefined),
  ).toEqual([area(0, 0, 6, 4), area(6, 0, 6, 4), area(0, 4, 12, 2)]);
});

test("a grid of 6 columns halves the widgets and flows them without holes", () => {
  const quarters = [area(0, 0, 3, 4), area(3, 0, 3, 4), area(6, 0, 3, 4), area(9, 0, 3, 4)];
  expect(arrangeLayouts(quarters, 6)).toEqual([
    area(0, 0, 2, 4),
    area(2, 0, 2, 4),
    area(4, 0, 2, 4),
    area(0, 4, 2, 4),
  ]);
  const besideTall = [area(0, 0, 4, 6), area(6, 0, 3, 4), area(9, 0, 3, 4), area(0, 6, 3, 4)];
  expect(arrangeLayouts(besideTall, 6)).toEqual([
    area(0, 0, 2, 6),
    area(2, 0, 2, 4),
    area(4, 0, 2, 4),
    area(2, 4, 2, 4),
  ]);
});

test("a phone puts the small widgets two in a row and moves the others up", () => {
  const layouts = [area(0, 0, 3, 4), area(3, 0, 3, 4), area(6, 0, 6, 6), area(0, 10, 3, 4)];
  expect(arrangeLayouts(layouts, 2)).toEqual([
    area(0, 0, 1, 4),
    area(1, 0, 1, 4),
    area(0, 4, 2, 6),
    area(0, 10, 1, 4),
  ]);
  expect(arrangeLayouts(layouts, 12)).toEqual(layouts);
});

test("a widget stays inside the grid as it grows", () => {
  expect(resizeLayout(area(9, 0, 3, 4), 6, 30)).toEqual(area(6, 0, 6, 16));
  expect(pickGridColumnCount(1200)).toBe(12);
  expect(pickGridColumnCount(800)).toBe(6);
  expect(pickGridColumnCount(400)).toBe(2);
});
