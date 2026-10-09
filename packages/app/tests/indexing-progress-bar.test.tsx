import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page } from "vitest/browser";

import type { Api, Indexing } from "@otelo/api";
import { expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toServices } from "./fixtures";
import { mountApp } from "./mount";

interface IndexingEvents extends Pick<Api, "subscribeToIndexing"> {
  readonly showIndexing: (indexing: Indexing) => void;
}

test("the bar shows how far the index has read, and the page reads again once it caught up", async () => {
  const catchingUp: Indexing = {
    logs: { state: "catching_up", indexed_percent: 40 },
    spans: { state: "caught_up" },
    metrics: { state: "catching_up", indexed_percent: 20 },
  };
  const caughtUp: Indexing = {
    logs: { state: "caught_up" },
    spans: { state: "caught_up" },
    metrics: { state: "caught_up" },
  };
  const { subscribeToIndexing, showIndexing } = createIndexingEvents();
  let servicesAnswer = toServices(["api"]);
  const api = createFakeApi({
    getServices: () => Promise.resolve(servicesAnswer),
    subscribeToIndexing,
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/services");
    await expect.element(page.getByRole("row", { name: /api/ })).toBeInTheDocument();
    showIndexing(catchingUp);
    await expect
      .element(page.getByRole("progressbar", { name: "Indexing the journal" }))
      .toHaveAttribute("aria-valuenow", "53");
    await expect
      .element(
        page.getByText("Indexing the journal. Pages show the newest telemetry once it finishes."),
      )
      .toBeInTheDocument();
    await expect.element(page.getByText("53 %")).toBeInTheDocument();

    servicesAnswer = toServices(["api", "worker"]);
    showIndexing(caughtUp);
    await expect.element(page.getByRole("progressbar")).not.toBeInTheDocument();
    await expect.element(page.getByText("53 %")).not.toBeInTheDocument();
    await expect.element(page.getByRole("row", { name: /worker/ })).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("an index that was caught up all along reads nothing again", async () => {
  const { subscribeToIndexing, showIndexing } = createIndexingEvents();
  let servicesCalls = 0;
  const api = createFakeApi({
    getServices: () => {
      servicesCalls += 1;
      return Promise.resolve(toServices(["api"]));
    },
    subscribeToIndexing,
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/services");
    await expect.element(page.getByRole("row", { name: /api/ })).toBeInTheDocument();
    showIndexing({
      logs: { state: "starting" },
      spans: { state: "starting" },
      metrics: { state: "starting" },
    });
    showIndexing({
      logs: { state: "caught_up" },
      spans: { state: "caught_up" },
      metrics: { state: "caught_up" },
    });
    await expect.element(page.getByRole("progressbar")).not.toBeInTheDocument();
  });

  expect(servicesCalls).toBe(1);
  expectNoReactivityMistakes(artifact);
});

function createIndexingEvents(): IndexingEvents {
  const listeners = new Set<(indexing: Indexing) => void>();
  return {
    subscribeToIndexing: (onIndexing) => {
      listeners.add(onIndexing);
      return () => listeners.delete(onIndexing);
    },
    showIndexing: (indexing) => {
      for (const listener of listeners) listener(indexing);
    },
  };
}
