import { captureArtifact, expectRerunBudget } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page } from "vitest/browser";

import type { Dashboard, Widget } from "@otelo/api";
import {
  expectNoReactivityMistakes,
  movePointerTo,
  type PointerPoint,
  pressPointerAt,
  readCenter,
  releasePointer,
} from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { mountApp } from "./mount";

const NOTES: ReadonlyArray<Widget> = ["Deploys", "On call", "Runbooks"].map((title, index) => ({
  title,
  layout: { column: index * 4, row: 0, width: 4, height: 4 },
  display: { kind: "note", text: `${title} notes` },
}));

const DASHBOARD: Dashboard = {
  id: 1,
  created_at: "2026-10-01T00:00:00Z",
  updated_at: "2026-10-01T00:00:00Z",
  definition: { name: "Team", description: "", widgets: [...NOTES] },
};

test("dragging a widget within one cell moves only that widget", async () => {
  const api = createFakeApi({ getDashboard: () => Promise.resolve(DASHBOARD) });
  mountApp(api, "/dashboards/1");
  await expect.element(page.getByText("Deploys notes")).toBeInTheDocument();
  const grabbed = readHeaderCenter("Deploys");
  await pressPointerAt(grabbed);
  await movePointerTo({ x: grabbed.x + 2, y: grabbed.y });

  const { artifact } = await captureArtifact(async () => {
    await movePointerTo({ x: grabbed.x + 3, y: grabbed.y });
    await movePointerTo({ x: grabbed.x + 4, y: grabbed.y });
    await movePointerTo({ x: grabbed.x + 5, y: grabbed.y });
  });
  await releasePointer();

  expectRerunBudget(artifact, 3);
  expectNoReactivityMistakes(artifact);
});

test("dropping a widget in another cell leaves the dashboard with unsaved changes", async () => {
  const api = createFakeApi({ getDashboard: () => Promise.resolve(DASHBOARD) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/dashboards/1");
    await expect.element(page.getByText("Deploys notes")).toBeInTheDocument();
    const grabbed = readHeaderCenter("Deploys");
    await pressPointerAt(grabbed);
    await movePointerTo({ x: grabbed.x + 2, y: grabbed.y });
    await movePointerTo({ x: grabbed.x + 400, y: grabbed.y });
    await releasePointer();
    await expect.element(page.getByText("Unsaved changes")).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("resizing a widget by its corner leaves the dashboard with unsaved changes", async () => {
  const api = createFakeApi({ getDashboard: () => Promise.resolve(DASHBOARD) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/dashboards/1");
    const corner = page.getByRole("button", { name: /^Resize Runbooks/ });
    await expect.element(corner).toBeInTheDocument();
    const grabbed = readCenter(corner.element());
    await pressPointerAt(grabbed);
    await movePointerTo({ x: grabbed.x, y: grabbed.y + 200 });
    await expect.element(page.getByText("4 × 8")).toBeInTheDocument();
    await releasePointer();
    await expect.element(page.getByText("Unsaved changes")).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

function readHeaderCenter(title: string): PointerPoint {
  const header = page.getByRole("heading", { name: title }).element().closest("header");
  if (!header) throw new Error(`The ${title} widget has no header`);
  return readCenter(header);
}
