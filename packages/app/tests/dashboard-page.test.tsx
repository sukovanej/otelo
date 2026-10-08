import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import type { Dashboard, DashboardDefinition } from "@otelo/api";
import { expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { mountApp } from "./mount";

const SAVED_DEFINITION: DashboardDefinition = {
  name: "Latency",
  description: "",
  widgets: [],
};

test("a dashboard keeps its edits apart from what it saved, through reloads, a discard and a save", async () => {
  const sentDefinitions: DashboardDefinition[] = [];
  let storedDefinition = SAVED_DEFINITION;
  const api = createFakeApi({
    getDashboard: () => Promise.resolve(toDashboard(storedDefinition)),
    replaceDashboard: (_id, definition) => {
      storedDefinition = definition;
      sentDefinitions.push(definition);
      return Promise.resolve(toDashboard(definition));
    },
  });
  const nameInput = page.getByLabelText("Name");
  const unsavedChanges = page.getByText("Unsaved changes");

  const { artifact } = await captureArtifact(async () => {
    const { queryClient } = mountApp(api, "/dashboards/1");
    await expect.element(nameInput).toHaveValue("Latency");

    await queryClient.refetchQueries();
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();

    await userEvent.fill(nameInput, "Latency by route");
    await expect.element(unsavedChanges).toBeInTheDocument();
    await queryClient.refetchQueries();
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
    await expect.element(nameInput).toHaveValue("Latency by route");
    await userEvent.click(page.getByRole("button", { name: "Discard", exact: true }));
    await expect.element(unsavedChanges).not.toBeInTheDocument();
    await expect.element(nameInput).toHaveValue("Latency");

    await userEvent.fill(nameInput, "Latency by route");
    await expect.element(unsavedChanges).toBeInTheDocument();
    await userEvent.click(page.getByRole("button", { name: "Save", exact: true }));
    await expect.element(unsavedChanges).not.toBeInTheDocument();
    await expect.element(nameInput).toHaveValue("Latency by route");
  });

  expect(sentDefinitions).toEqual([{ ...SAVED_DEFINITION, name: "Latency by route" }]);
  expectNoReactivityMistakes(artifact);
});

function toDashboard(definition: DashboardDefinition): Dashboard {
  return {
    id: 1,
    created_at: "2026-10-01T00:00:00Z",
    updated_at: "2026-10-01T00:00:00Z",
    definition,
  };
}
