import { captureArtifact, expectRerunBudget } from "@solidjs/diagnostics";
import { createSignal, flush } from "solid-js";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes, mountView } from "@otelo/testing";

import { Select, type SelectOption } from "../src";

const COUNT_OPTIONS: ReadonlyArray<SelectOption<string>> = Array.from(
  { length: 40 },
  (_, index) => ({ value: String(index + 1), label: String(index + 1) }),
);

test("a value that changes while the list is open costs two runs a row", async () => {
  const [value, setValue] = createSignal("1");
  mountView(() => (
    <Select
      selection="single"
      label="Top"
      options={COUNT_OPTIONS}
      value={value()}
      onChange={setValue}
    />
  ));
  await userEvent.click(page.getByRole("combobox", { name: "Top" }));
  await expect
    .element(page.getByRole("option", { name: "1", exact: true }))
    .toHaveAttribute("aria-selected", "true");

  const { artifact } = await captureArtifact(() => {
    setValue("2");
    flush();
  });

  await expect
    .element(page.getByRole("option", { name: "2", exact: true }))
    .toHaveAttribute("aria-selected", "true");
  expectRerunBudget(artifact, 1 + 2 * COUNT_OPTIONS.length);
  expectNoReactivityMistakes(artifact);
});
