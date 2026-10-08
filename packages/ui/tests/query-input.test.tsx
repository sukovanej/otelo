import { captureArtifact, expectRerunBudget } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes, mountView } from "@otelo/testing";

import { QueryInput } from "../src";
import type { Suggestion } from "../src/completion";

const FIELD_SUGGESTIONS: ReadonlyArray<Suggestion> = Array.from({ length: 12 }, (_, index) => ({
  text: `service.attribute_${index}`,
  start: 0,
  end: 1,
  kind: "attribute",
  detail: null,
}));

test("moving through the suggestions redraws only the two that change, and the field", async () => {
  mountView(() => (
    <QueryInput
      label="Query"
      value=""
      highlight={(query) => [{ kind: "undecided", start: 0, end: Array.from(query).length }]}
      complete={() => Promise.resolve([...FIELD_SUGGESTIONS])}
      help={() => Promise.resolve(undefined)}
      onInput={() => {}}
      onSubmit={() => {}}
    />
  ));
  await userEvent.type(page.getByRole("combobox", { name: "Query" }), "s");
  await expect.element(page.getByRole("option").first()).toBeInTheDocument();
  await userEvent.keyboard("{ArrowDown}");
  await expect.element(page.getByRole("option").first()).toHaveAttribute("aria-selected", "true");

  const { artifact } = await captureArtifact(async () => {
    await userEvent.keyboard("{ArrowDown}");
    await expect.element(page.getByRole("option").nth(1)).toHaveAttribute("aria-selected", "true");
  });

  expectRerunBudget(artifact, 4);
  expectNoReactivityMistakes(artifact);
});
