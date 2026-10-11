import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import type { Api, Traces } from "@otelo/api";
import { createHeldAnswers, expectNoReactivityMistakes, type HeldAnswers } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toTraces, toTraceSummary } from "./fixtures";
import { mountApp } from "./mount";
import { switchToPhoneScreen } from "./screen";

const HOUR_MS = 3_600_000;

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

test("on a phone, stepping the range back past midnight while the traces load shows the day", async () => {
  await switchToPhoneScreen();
  const { api, traces } = createHeldTracesApi();
  const todayAtHalfPastMidnight = new Date();
  todayAtHalfPastMidnight.setHours(0, 30, 0, 0);
  const since = todayAtHalfPastMidnight.toISOString();
  const until = new Date(todayAtHalfPastMidnight.getTime() + HOUR_MS).toISOString();
  const earlier = page.getByRole("button", { name: /^Earlier/ });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, `/traces?since=${encodeURIComponent(since)}&until=${encodeURIComponent(until)}`);
    await expect.element(page.getByText("2 traces")).toBeInTheDocument();

    await userEvent.click(earlier);
    await userEvent.click(earlier);
    await expect
      .element(page.getByRole("button", { name: /^Range: Yesterday/ }))
      .toBeInTheDocument();
    await expect.poll(() => traces.held.length).toBe(1);

    traces.releaseAll(SORTED_TRACES);
    await expect.element(page.getByText("1 trace")).toBeInTheDocument();
    await expect
      .element(page.getByRole("button", { name: /^Range:/ }))
      .toHaveTextContent("Yesterday22:30 – 23:30");
  });

  expectNoReactivityMistakes(artifact);
});

test("quick clicks on Earlier ask for the traces of the range they end on, once", async () => {
  const { api, sentSorts, traces } = createHeldTracesApi();
  const earlier = page.getByRole("button", { name: /^Earlier/ });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/traces");
    await expect.element(page.getByText("2 traces")).toBeInTheDocument();

    await userEvent.click(earlier);
    await userEvent.click(earlier);
    await userEvent.click(earlier);
    await expect.element(page.getByText("Loading…")).toBeInTheDocument();
    await expect.poll(() => traces.held.length).toBe(1);

    traces.releaseAll(SORTED_TRACES);
    await expect.element(page.getByText("1 trace")).toBeInTheDocument();
  });

  expect(sentSorts).toHaveLength(2);
  expect(artifact.diagnostics.map((event) => event.code)).not.toContain("ABANDONED_FLIGHTS");
  expectNoReactivityMistakes(artifact);
});

// solid-query relays a key change to its observer from a render effect, which tears one frame
// when the key returns to a cached answer: TanStack/query#11903. This fails once it is fixed.
test.fails("Now after a step back shows the traces that end now again", async () => {
  const { api, traces } = createHeldTracesApi();

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/traces");
    await expect.element(page.getByText("2 traces")).toBeInTheDocument();

    await userEvent.click(page.getByRole("button", { name: /^Earlier/ }));
    await expect.poll(() => traces.held.length).toBe(1);
    traces.releaseAll(SORTED_TRACES);
    await expect.element(page.getByText("1 trace")).toBeInTheDocument();

    await userEvent.click(page.getByRole("button", { name: "Now" }));
    await expect.element(page.getByText("2 traces")).toBeInTheDocument();
    await expect.poll(() => traces.held.length).toBe(2);
    traces.releaseAll(FIRST_TRACES);
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
