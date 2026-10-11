import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toTrace, toTraceSpan } from "./fixtures";
import { mountApp } from "./mount";

const TRACE = toTrace([
  toTraceSpan("root", null, 0, 100),
  toTraceSpan("a", "root", 0, 40),
  toTraceSpan("a1", "a", 5, 20),
  toTraceSpan("b", "root", 40, 60),
  toTraceSpan("b1", "b", 45, 10),
  toTraceSpan("b2", "b", 60, 30),
]);

test("folding a span hides the spans under it", async () => {
  const api = createFakeApi({ getTrace: () => Promise.resolve(TRACE) });
  mountApp(api, "/traces/t1");
  await expect.element(page.getByText("step a1")).toBeInTheDocument();
  const foldA = page
    .getByRole("row")
    .filter({ hasText: "step a" })
    .first()
    .getByRole("button", { name: "Fold" });

  const { artifact } = await captureArtifact(async () => {
    await userEvent.click(foldA);
    await expect.element(page.getByText("step a1")).not.toBeInTheDocument();
    await expect.element(page.getByText("+1")).toBeInTheDocument();
    await expect.element(page.getByText("step b2")).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("the open span shows its kind and its status as badges", async () => {
  const failedSpan = { ...toTraceSpan("root", null, 0, 100), kind: 2, status: 2 };
  const api = createFakeApi({ getTrace: () => Promise.resolve(toTrace([failedSpan])) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/traces/t1?span=root");

    await expect.element(fieldValue("kind", "server")).toHaveClass(/text-info/);
    await expect.element(fieldValue("status", "error")).toHaveClass(/text-error/);
  });

  expectNoReactivityMistakes(artifact);
});

function fieldValue(fieldName: string, value: string) {
  return page
    .getByRole("row")
    .filter({ has: page.getByRole("rowheader", { name: fieldName, exact: true }) })
    .getByText(value, { exact: true });
}
