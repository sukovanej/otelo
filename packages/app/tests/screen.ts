import { expect, onTestFinished } from "vitest";
import { type Locator, page } from "vitest/browser";

const PHONE_SCREEN = { width: 375, height: 812 };

const DESKTOP_SCREEN = { width: 1280, height: 800 };

const PAGE_GUTTER_PX = 16;

const LAYOUT_TOLERANCE_PX = 1;

export async function switchToPhoneScreen(): Promise<void> {
  await page.viewport(PHONE_SCREEN.width, PHONE_SCREEN.height);
  onTestFinished(() => page.viewport(DESKTOP_SCREEN.width, DESKTOP_SCREEN.height));
}

export function expectOnOneRow(first: Locator, second: Locator): void {
  const [firstBox, secondBox] = [readBox(first), readBox(second)];
  expect(Math.abs(findMiddleY(firstBox) - findMiddleY(secondBox))).toBeLessThan(
    LAYOUT_TOLERANCE_PX * 2,
  );
}

export function expectInsidePageGutters(locator: Locator): void {
  const box = readBox(locator);
  expect(box.left).toBeGreaterThanOrEqual(PAGE_GUTTER_PX - LAYOUT_TOLERANCE_PX);
  expect(box.right).toBeLessThanOrEqual(window.innerWidth - PAGE_GUTTER_PX + LAYOUT_TOLERANCE_PX);
}

export function expectInsideScreen(locator: Locator): void {
  const box = readBox(locator);
  expect(box.left).toBeGreaterThanOrEqual(0);
  expect(box.right).toBeLessThanOrEqual(window.innerWidth);
}

export function expectToFillScreen(locator: Locator): void {
  const box = readBox(locator);
  expect([box.left, box.top, box.width, box.height]).toEqual([
    0,
    0,
    window.innerWidth,
    window.innerHeight,
  ]);
}

export function readBox(locator: Locator): DOMRect {
  return locator.element().getBoundingClientRect();
}

function findMiddleY(box: DOMRect): number {
  return box.top + box.height / 2;
}
