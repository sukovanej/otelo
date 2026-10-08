import { captureArtifact } from "@solidjs/diagnostics";
import { createSignal, Show } from "solid-js";
import { expect, onTestFinished, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes, mountView } from "@otelo/testing";

import { ActionMenu, type ActionMenuItem } from "../src";

const ITEMS: ReadonlyArray<ActionMenuItem> = [
  { label: "Clone", icon: () => null, tone: "default", onSelect: () => {} },
];

// Removing an open popover hides it without a toggle event, so the menu has nothing visible
// left to show whether it still listens; the window's own calls are what tell.
test("a menu removed while open stops listening to the window", async () => {
  const addListener = vi.spyOn(window, "addEventListener");
  const removeListener = vi.spyOn(window, "removeEventListener");
  onTestFinished(() => {
    vi.restoreAllMocks();
  });
  const [isShown, setShown] = createSignal(true);

  const { artifact } = await captureArtifact(async () => {
    mountView(() => (
      <Show when={isShown()}>
        <ActionMenu label="Widget actions" items={ITEMS} />
      </Show>
    ));
    await userEvent.click(page.getByRole("button", { name: "Widget actions" }));
    await expect.element(page.getByRole("menu", { name: "Widget actions" })).toBeVisible();
    setShown(false);
    await expect
      .element(page.getByRole("menu", { name: "Widget actions" }))
      .not.toBeInTheDocument();
  });

  expect(listWindowListeners(addListener.mock.calls)).toEqual(
    listWindowListeners(removeListener.mock.calls),
  );
  expectNoReactivityMistakes(artifact);
});

function listWindowListeners(calls: ReadonlyArray<ReadonlyArray<unknown>>): string[] {
  return calls
    .map(([type]) => String(type))
    .filter((type) => type === "scroll" || type === "resize")
    .toSorted();
}
