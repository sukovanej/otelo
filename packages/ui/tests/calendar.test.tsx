import { captureArtifact, expectRerunBudget } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes, mountView } from "@otelo/testing";

import Calendar from "../src/RangePicker/calendar";

test("moving the focus a day redraws only the two days it moves between", async () => {
  mountView(() => <Calendar label="Start" value="2026-07-15" onChange={() => {}} />);
  await userEvent.click(page.getByRole("button", { name: "Wednesday 15 July 2026" }));

  const { artifact } = await captureArtifact(async () => {
    await userEvent.keyboard("{ArrowRight}");
    await expect.element(page.getByRole("button", { name: "Thursday 16 July 2026" })).toHaveFocus();
  });

  expectRerunBudget(artifact, 4);
  expectNoReactivityMistakes(artifact);
});
