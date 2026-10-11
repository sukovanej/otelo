import { captureArtifact } from "@solidjs/diagnostics";
import { createSignal } from "solid-js";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes, mountView } from "@otelo/testing";

import RangePicker from "../src/RangePicker";

test("the fields of an open picker follow a step to an earlier range", async () => {
  const [since, setSince] = createSignal(new Date(2026, 8, 30, 8).toISOString());
  const [until, setUntil] = createSignal(new Date(2026, 8, 30, 9).toISOString());
  mountView(() => (
    <RangePicker
      since={since()}
      until={until()}
      onChange={(nextSince, nextUntil) => {
        setSince(nextSince);
        setUntil(nextUntil);
      }}
    />
  ));
  await userEvent.click(page.getByRole("button", { name: /^Range:/ }));
  await expect.element(page.getByRole("textbox", { name: "From time" })).toHaveValue("08:00:00");

  const { artifact } = await captureArtifact(async () => {
    await userEvent.click(page.getByRole("button", { name: /^Earlier/ }));
    await expect.element(page.getByRole("textbox", { name: "From time" })).toHaveValue("07:00:00");
    await expect.element(page.getByRole("textbox", { name: "To time" })).toHaveValue("08:00:00");
  });

  expectNoReactivityMistakes(artifact);
});
