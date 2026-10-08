import { captureArtifact } from "@solidjs/diagnostics";
import { createRouter, memoryHistory } from "@solidjs/router";
import { render } from "@solidjs/web";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { afterEach, expect, test, vi } from "vitest";

import type { Dashboard, DashboardDefinition } from "@otelo/api";

import DashboardPage from "../src/dashboards/DashboardPage";
import { expectNoReactivityMistakes } from "./diagnostics";

const SAVED_DEFINITION: DashboardDefinition = {
  name: "Latency",
  description: "",
  widgets: [],
};

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

test("a dashboard keeps its edits apart from what it saved, through reloads, a discard and a save", async () => {
  const sentDefinitions: DashboardDefinition[] = [];
  let storedDefinition = SAVED_DEFINITION;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (_path: string, init: RequestInit) => {
      if (init.method === "PUT" && typeof init.body === "string") {
        storedDefinition = parseDefinition(init.body);
        sentDefinitions.push(storedDefinition);
      }
      return Response.json(toDashboard(storedDefinition));
    }),
  );
  const container = document.body.appendChild(document.createElement("div"));
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const Router = createRouter({
    routes: [{ path: "/dashboards/:id", component: DashboardPage }],
    history: memoryHistory("/dashboards/1"),
  });

  const { artifact } = await captureArtifact(async () => {
    render(
      () => (
        <QueryClientProvider client={queryClient}>
          <Router />
        </QueryClientProvider>
      ),
      container,
    );
    const nameInput = await vi.waitFor(() => {
      const input = container.querySelector<HTMLInputElement>('input[aria-label="Name"]');
      expect(input?.value).toBe("Latency");
      return input;
    });

    await reloadDashboard(queryClient, container);

    typeName(nameInput, "Latency by route");
    await vi.waitFor(() => expect(container.textContent).toContain("Unsaved changes"));
    await reloadDashboard(queryClient, container);
    expect(nameInput?.value).toBe("Latency by route");
    clickButton(container, "Discard");
    await vi.waitFor(() => expect(container.textContent).not.toContain("Unsaved changes"));
    expect(nameInput?.value).toBe("Latency");

    typeName(nameInput, "Latency by route");
    await vi.waitFor(() => expect(container.textContent).toContain("Unsaved changes"));
    clickButton(container, "Save");
    await vi.waitFor(() => expect(container.textContent).not.toContain("Unsaved changes"));
    expect(nameInput?.value).toBe("Latency by route");
  });

  expect(sentDefinitions).toEqual([{ ...SAVED_DEFINITION, name: "Latency by route" }]);
  expectNoReactivityMistakes(artifact);
});

async function reloadDashboard(queryClient: QueryClient, container: HTMLElement) {
  await queryClient.refetchQueries();
  await vi.waitFor(() => expect(container.textContent).not.toContain("Loading…"));
}

function typeName(input: HTMLInputElement | null, name: string) {
  if (!input) throw new Error("The page has no name field");
  input.value = name;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function clickButton(container: HTMLElement, label: string) {
  const button = [...container.querySelectorAll("button")].find(
    (candidate) => candidate.textContent.trim() === label,
  );
  if (!button) throw new Error(`The page has no ${label} button`);
  button.click();
}

function toDashboard(definition: DashboardDefinition): Dashboard {
  return {
    id: 1,
    created_at: "2026-10-01T00:00:00Z",
    updated_at: "2026-10-01T00:00:00Z",
    definition,
  };
}

function parseDefinition(body: string): DashboardDefinition {
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- the page sends a definition
  return JSON.parse(body) as DashboardDefinition;
}
