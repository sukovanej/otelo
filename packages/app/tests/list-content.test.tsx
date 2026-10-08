import { captureArtifact, expectRerunBudget } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page } from "vitest/browser";

import { expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toLogLine, toLogs } from "./fixtures";
import { mountApp } from "./mount";

const INFO_SEVERITY = 9;

test("a reload of a list with nothing more to show leaves the watch on its end alone", async () => {
  const api = createFakeApi({
    getLogs: () => Promise.resolve(toLogs([toLogLine("cache warmed", "api", INFO_SEVERITY)])),
  });
  const { queryClient } = mountApp(api, "/logs");
  await expect.element(page.getByText("cache warmed")).toBeInTheDocument();

  const { artifact } = await captureArtifact(async () => {
    await queryClient.refetchQueries();
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
  });

  expectRerunBudget(artifact, 0, { scope: "watchListEnd" });
  expectNoReactivityMistakes(artifact);
});
