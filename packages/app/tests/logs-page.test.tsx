import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { createHeldAnswers, expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toDashboardList, toLogLine, toLogs } from "./fixtures";
import { mountApp } from "./mount";
import { expectNavigationToReach } from "./navigation";
import { switchToPhoneScreen } from "./screen";

const ERROR_SEVERITY = 17;

const SHOWN_LOGS = toLogs([toLogLine("payment declined", "api", ERROR_SEVERITY)]);

test("a second filter click while the first one loads keeps both terms", async () => {
  const logs = createHeldAnswers(SHOWN_LOGS);
  const api = createFakeApi({ getLogs: (_query, signal) => logs.answer(signal) });

  const { artifact } = await captureArtifact(async () => {
    const { history } = mountApp(api, "/logs");
    await userEvent.click(page.getByText("payment declined"));

    await filterField("service", 'Keep lines where service = "api"');
    await filterField("level", "Drop lines where level = error");

    logs.releaseAll(SHOWN_LOGS);
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
    const shownQuery = new URL(history.get(), "http://otelo").searchParams.get("q");
    expect(shownQuery).toContain('service = "api"');
    expect(shownQuery).toContain("level != error");
  });

  expectNoReactivityMistakes(artifact);
});

test("the open line shows its level as a badge", async () => {
  const api = createFakeApi({ getLogs: () => Promise.resolve(SHOWN_LOGS) });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/logs");
    await userEvent.click(page.getByText("payment declined"));

    const levelRow = page
      .getByRole("row")
      .filter({ has: page.getByRole("rowheader", { name: "level", exact: true }) });
    await expect.element(levelRow.getByText("ERROR", { exact: true })).toHaveClass(/text-error/);
  });

  expectNoReactivityMistakes(artifact);
});

test("leaving the logs while a range change loads keeps the next pages navigating", async () => {
  const logs = createHeldAnswers(SHOWN_LOGS);
  const api = createFakeApi({
    getLogs: (_query, signal) => logs.answer(signal),
    listDashboards: () => Promise.resolve(toDashboardList(["Latency"])),
  });

  const { artifact } = await captureArtifact(async () => {
    const app = mountApp(api, "/logs");
    await expect.element(page.getByText("payment declined")).toBeInTheDocument();
    await userEvent.click(page.getByRole("button", { name: /^Earlier by/ }));
    await expect.poll(() => logs.held.length).toBe(1);

    await expectNavigationToReach(app, "Dashboards", "/dashboards");
    logs.releaseAll(SHOWN_LOGS);
    await expectNavigationToReach(app, "Logs", "/logs");
  });

  expectNoReactivityMistakes(artifact);
});

test("on a phone an open line covers the lines, which come back when it closes", async () => {
  await switchToPhoneScreen();
  const api = createFakeApi({ getLogs: () => Promise.resolve(SHOWN_LOGS) });
  const lines = page.getByRole("table", { name: "Log lines" });
  const linePanel = page.getByRole("complementary", { name: "Log line" });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/logs");
    await expect.element(lines.getByText("payment declined")).toBeVisible();
    await expect
      .element(lines.getByRole("columnheader", { name: "Service" }))
      .not.toBeInTheDocument();

    await userEvent.click(lines.getByText("payment declined"));
    await expect.element(linePanel).toBeVisible();
    expect(linePanel.element().getBoundingClientRect().width).toBe(window.innerWidth);

    await userEvent.click(linePanel.getByRole("button", { name: "Close" }));
    await expect.element(linePanel).not.toBeInTheDocument();
    await expect.element(lines.getByText("payment declined")).toBeVisible();
  });

  expectNoReactivityMistakes(artifact);
});

async function filterField(fieldName: string, buttonTitle: string) {
  await userEvent.hover(page.getByRole("rowheader", { name: fieldName, exact: true }));
  await userEvent.click(page.getByTitle(buttonTitle));
}
