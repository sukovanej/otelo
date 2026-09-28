import { expect, test } from "vitest";
import { listStep, move } from "../src/keys";

const key = (
  name: string,
  mods: Partial<Record<"ctrlKey" | "altKey" | "metaKey" | "shiftKey", boolean>> = {},
) => ({
  key: name,
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  shiftKey: false,
  ...mods,
});

test("listStep reads the arrows and Ctrl+J and Ctrl+K", () => {
  expect(listStep(key("ArrowDown"))).toBe(1);
  expect(listStep(key("ArrowUp"))).toBe(-1);
  expect(listStep(key("j", { ctrlKey: true }))).toBe(1);
  expect(listStep(key("k", { ctrlKey: true }))).toBe(-1);
  expect(listStep(key("J", { ctrlKey: true }))).toBe(1);
  expect(listStep(key("j"))).toBe(0);
  expect(listStep(key("j", { metaKey: true }))).toBe(0);
  expect(listStep(key("k", { ctrlKey: true, shiftKey: true }))).toBe(0);
  expect(listStep(key("n", { ctrlKey: true }))).toBe(0);
});

test("move wraps at the ends", () => {
  expect(move(0, 1, 3)).toBe(1);
  expect(move(2, 1, 3)).toBe(0);
  expect(move(0, -1, 3)).toBe(2);
  expect(move(0, 1, 0)).toBe(0);
});

test("move with none stops at -1 between the ends", () => {
  expect(move(-1, 1, 3, true)).toBe(0);
  expect(move(2, 1, 3, true)).toBe(-1);
  expect(move(-1, -1, 3, true)).toBe(2);
  expect(move(0, -1, 3, true)).toBe(-1);
  expect(move(-1, 1, 0, true)).toBe(-1);
});
