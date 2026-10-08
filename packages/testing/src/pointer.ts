import { commands } from "vitest/browser";

declare module "vitest/browser" {
  interface BrowserCommands {
    movePointer: (x: number, y: number) => Promise<void>;
    pressPointer: () => Promise<void>;
    releasePointer: () => Promise<void>;
  }
}

export interface PointerPoint {
  readonly x: number;
  readonly y: number;
}

export function readCenter(element: Element): PointerPoint {
  const box = element.getBoundingClientRect();
  return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
}

export async function pressPointerAt(point: PointerPoint): Promise<void> {
  await commands.movePointer(point.x, point.y);
  await commands.pressPointer();
}

export async function movePointerTo(point: PointerPoint): Promise<void> {
  await commands.movePointer(point.x, point.y);
}

export async function releasePointer(): Promise<void> {
  await commands.releasePointer();
}
