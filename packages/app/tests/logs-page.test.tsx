import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { createHeldAnswers, expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toDashboardList, toLogLine, toLogs } from "./fixtures";
import { mountApp } from "./mount";
import { expectNavigationToReach } from "./navigation";

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

    await expectNavigationToReach(app, "Dashboards", "/dashboards");
    logs.releaseAll(SHOWN_LOGS);
    await expectNavigationToReach(app, "Logs", "/logs");
  });

  expectNoReactivityMistakes(artifact);
});

async function filterField(fieldName: string, buttonTitle: string) {
  await userEvent.hover(page.getByRole("rowheader", { name: fieldName, exact: true }));
  await userEvent.click(page.getByTitle(buttonTitle));
}
