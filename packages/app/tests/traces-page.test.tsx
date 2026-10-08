import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import type { Api, Traces } from "@otelo/api";
import { createHeldAnswers, expectNoReactivityMistakes, type HeldAnswers } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toTraces, toTraceSummary } from "./fixtures";
import { mountApp } from "./mount";

const FIRST_TRACES = toTraces([toTraceSummary("a1", 2e6), toTraceSummary("b2", 9e6)]);

const SORTED_TRACES = toTraces([toTraceSummary("c3", 5e6)]);

test("the sort header follows each click while the sorted traces load", async () => {
  const { api, sentSorts, traces } = createHeldTracesApi();
  const durationHeader = page.getByRole("columnheader", { name: "Duration" });

  const { artifact } = await captureArtifact(async () => {
    const { history } = mountApp(api, "/traces");
    await expect.element(page.getByText("2 traces")).toBeInTheDocument();

    await userEvent.click(durationHeader.getByRole("button"));
    await expect.element(durationHeader).toHaveAttribute("aria-sort", "descending");
    await userEvent.click(durationHeader.getByRole("button"));
    await expect.element(durationHeader).toHaveAttribute("aria-sort", "ascending");

    traces.releaseAll(SORTED_TRACES);
    await expect.element(page.getByText("1 trace")).toBeInTheDocument();
    expect(new URL(history.get(), "http://otelo").searchParams.get("sort")).toBe("shortest");
  });

  expect(sentSorts.at(-1)).toBe("shortest");
  expectNoReactivityMistakes(artifact);
});

test("the loading notice leaves once the sorted traces land", async () => {
  const { api, traces } = createHeldTracesApi();

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/traces");
    await expect.element(page.getByText("2 traces")).toBeInTheDocument();

    const durationHeader = page.getByRole("columnheader", { name: "Duration" });
    await userEvent.click(durationHeader.getByRole("button"));
    await expect.element(page.getByText("Loading…")).toBeInTheDocument();

    traces.releaseAll(SORTED_TRACES);
    await expect.element(page.getByText("1 trace")).toBeInTheDocument();
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

interface HeldTracesApi {
  readonly api: Api;
  readonly sentSorts: ReadonlyArray<string | undefined>;
  readonly traces: HeldAnswers<Traces>;
}

function createHeldTracesApi(): HeldTracesApi {
  const sentSorts: Array<string | undefined> = [];
  const traces = createHeldAnswers(FIRST_TRACES);
  const api = createFakeApi({
    getTraces: (query, signal) => {
      sentSorts.push(query.sort);
      return traces.answer(signal);
    },
  });
  return { api, sentSorts, traces };
}
