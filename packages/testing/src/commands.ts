/// <reference types="@vitest/browser-playwright" />

import type { BrowserCommand, BrowserCommandContext } from "vitest/node";

// The mouse of the browser, driven in the coordinates of the page under test, so a test can
// press, move and release it in as many steps as a drag takes.
export const pointerCommands = {
  movePointer: (async (context, x: number, y: number) => {
    const point = await toWindowPoint(context, x, y);
    await context.page.mouse.move(point.x, point.y);
  }) satisfies BrowserCommand<[x: number, y: number]>,
  pressPointer: (async (context) => {
    await context.page.mouse.down();
  }) satisfies BrowserCommand,
  releasePointer: (async (context) => {
    await context.page.mouse.up();
  }) satisfies BrowserCommand,
};

interface WindowPoint {
  readonly x: number;
  readonly y: number;
}

async function toWindowPoint(
  context: BrowserCommandContext,
  x: number,
  y: number,
): Promise<WindowPoint> {
  const frame = await context.frame();
  const frameBox = await (await frame.frameElement()).boundingBox();
  if (!frameBox) throw new Error("The page under test is not drawn");
  const frameWidth = await frame.evaluate(() => window.innerWidth);
  const scale = frameBox.width / frameWidth;
  return { x: frameBox.x + x * scale, y: frameBox.y + y * scale };
}
