import { expect } from "vitest";
import { page, userEvent } from "vitest/browser";

import type { MountedApp } from "./mount";

export async function expectNavigationToReach(app: MountedApp, linkName: string, path: string) {
  await userEvent.click(page.getByRole("navigation").getByRole("link", { name: linkName }));
  await expect.poll(() => app.history.get()).toBe(path);
}
