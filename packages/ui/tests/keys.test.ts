import { expect, test } from "vitest";

import { moveListIndex, NO_ITEM, toListStep } from "../src/keys";

test("toListStep reads the arrows and Ctrl+J and Ctrl+K", () => {
  expect(toListStep(pressKey("ArrowDown"))).toBe(1);
  expect(toListStep(pressKey("ArrowUp"))).toBe(-1);
  expect(toListStep(pressKey("j", { ctrlKey: true }))).toBe(1);
  expect(toListStep(pressKey("k", { ctrlKey: true }))).toBe(-1);
  expect(toListStep(pressKey("J", { ctrlKey: true }))).toBe(1);
  expect(toListStep(pressKey("j"))).toBe(0);
  expect(toListStep(pressKey("j", { metaKey: true }))).toBe(0);
  expect(toListStep(pressKey("k", { ctrlKey: true, shiftKey: true }))).toBe(0);
  expect(toListStep(pressKey("n", { ctrlKey: true }))).toBe(0);
});

test("moveListIndex wraps at the ends", () => {
  expect(moveListIndex(0, 1, 3)).toBe(1);
  expect(moveListIndex(2, 1, 3)).toBe(0);
  expect(moveListIndex(0, -1, 3)).toBe(2);
  expect(moveListIndex(0, 1, 0)).toBe(0);
});

test("moveListIndex with the no-item stop stops at NO_ITEM between the ends", () => {
  expect(moveListIndex(NO_ITEM, 1, 3, true)).toBe(0);
  expect(moveListIndex(2, 1, 3, true)).toBe(NO_ITEM);
  expect(moveListIndex(NO_ITEM, -1, 3, true)).toBe(2);
  expect(moveListIndex(0, -1, 3, true)).toBe(NO_ITEM);
  expect(moveListIndex(NO_ITEM, 1, 0, true)).toBe(NO_ITEM);
});

type ModifierKey = "ctrlKey" | "altKey" | "metaKey" | "shiftKey";

function pressKey(name: string, modifiers: Partial<Record<ModifierKey, boolean>> = {}) {
  return {
    key: name,
    ctrlKey: false,
    altKey: false,
    metaKey: false,
    shiftKey: false,
    ...modifiers,
  };
}
