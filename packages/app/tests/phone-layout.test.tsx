import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import {
  toLogLine,
  toLogs,
  toServices,
  toSpans,
  toTrace,
  toTraces,
  toTraceSpan,
  toTraceSummary,
} from "./fixtures";
import { mountApp } from "./mount";
import {
  expectInsidePageGutters,
  expectInsideScreen,
  expectOnOneRow,
  expectToFillScreen,
  readBox,
  switchToPhoneScreen,
} from "./screen";

const DAY_MS = 86_400_000;

const HOUR_MS = 3_600_000;

const TRACE = toTrace([toTraceSpan("root", null, 0, 100), toTraceSpan("charge", "root", 10, 60)]);

test("on a phone the traces keep their time, root span and duration", async () => {
  await switchToPhoneScreen();
  const api = createFakeApi({
    getTraces: () => Promise.resolve(toTraces([toTraceSummary("a1", 2e6)])),
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/traces");
    await expect.element(page.getByText("1 trace")).toBeInTheDocument();
  });

  expect(readColumnHeaders("Traces")).toEqual(["Time", "Root span", "Duration"]);
  expectNoReactivityMistakes(artifact);
});

test("on a phone the spans keep their time, name and duration", async () => {
  await switchToPhoneScreen();
  const api = createFakeApi({ getSpans: () => Promise.resolve(toSpans(TRACE.spans)) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/traces?view=spans");
    await expect.element(page.getByText("2 spans")).toBeInTheDocument();
  });

  expect(readColumnHeaders("Spans")).toEqual(["Time", "Name", "Duration"]);
  expectNoReactivityMistakes(artifact);
});

test("on a phone the services keep their requests, error rate and P95", async () => {
  await switchToPhoneScreen();
  const api = createFakeApi({ getServices: () => Promise.resolve(toServices(["api"])) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/services");
    await expect.element(page.getByRole("row", { name: /api/ })).toBeInTheDocument();
  });

  expect(readColumnHeaders("Services")).toEqual(["Service", "Requests", "Error rate", "P95"]);
  expectNoReactivityMistakes(artifact);
});

test("on a phone an open trace fills the screen, and its bar takes two rows", async () => {
  await switchToPhoneScreen();
  const api = createFakeApi({
    getTraces: () => Promise.resolve(toTraces([toTraceSummary("t1", 2e6)])),
    getTrace: () => Promise.resolve(TRACE),
  });
  const trace = page.getByRole("dialog", { name: "Trace" });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/traces");
    await userEvent.click(page.getByRole("table", { name: "Traces" }).getByRole("row").nth(1));
    await expect.element(trace.getByRole("row", { name: /step charge/ })).toBeInTheDocument();
  });

  expectToFillScreen(trace);
  expectOnOneRow(trace.getByRole("heading"), trace.getByRole("button", { name: "Close" }));
  expectOnOneRow(
    trace.getByRole("tab", { name: /Spans/ }),
    trace.getByRole("button", { name: "Reload" }),
  );
  expect(readColumnHeaders("Spans of the trace")).toEqual(["Span", "Duration", "Timeline"]);
  await expect.element(trace.getByText(/^Updated/)).not.toBeVisible();
  expectNoReactivityMistakes(artifact);
});

test("on a phone a range of another day fills its row with the range picker and Now", async () => {
  await switchToPhoneScreen();
  const api = createFakeApi({
    getLogs: () => Promise.resolve(toLogs([toLogLine("payment declined", "api", 9)])),
  });
  const yesterdayNoon = new Date(Date.now() - DAY_MS);
  yesterdayNoon.setHours(12, 0, 0, 0);
  const since = yesterdayNoon.toISOString();
  const until = new Date(yesterdayNoon.getTime() + HOUR_MS).toISOString();
  const earlier = page.getByRole("button", { name: /^Earlier/ });
  const now = page.getByRole("button", { name: "Now" });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, `/logs?since=${encodeURIComponent(since)}&until=${encodeURIComponent(until)}`);
    await expect.element(page.getByText("payment declined")).toBeInTheDocument();
  });

  await expect.element(page.getByRole("button", { name: /^Range: Yesterday/ })).toBeVisible();
  expectOnOneRow(earlier, now);
  expect(readBox(earlier).left).toBeCloseTo(readBox(page.getByRole("combobox")).left, 0);
  expect(readBox(now).right).toBeCloseTo(
    readBox(page.getByRole("button", { name: "Run" })).right,
    0,
  );
  expectInsidePageGutters(now);
  expectNoReactivityMistakes(artifact);
});

test("on a phone the range popover stays on the screen", async () => {
  await switchToPhoneScreen();
  const api = createFakeApi({ getServices: () => Promise.resolve(toServices(["api"])) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/services");
    await userEvent.click(page.getByRole("button", { name: /^Range:/ }));
    await expect.element(page.getByRole("dialog", { name: "Range" })).toBeVisible();
  });

  expectInsideScreen(page.getByRole("dialog", { name: "Range" }));
  expectNoReactivityMistakes(artifact);
});

function readColumnHeaders(tableName: string): string[] {
  return page
    .getByRole("table", { name: tableName })
    .or(page.getByRole("treegrid", { name: tableName }))
    .getByRole("columnheader")
    .elements()
    .map(
      (header) =>
        header.getAttribute("aria-label") ?? header.textContent.replace(/[↑↓]/g, "").trim(),
    );
}
