import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { createHeldAnswers, expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toDashboardList, toServices } from "./fixtures";
import { mountApp } from "./mount";
import { expectNavigationToReach } from "./navigation";

const SERVICES = toServices(["api"]);

test("the range follows each click while the services load", async () => {
  const services = createHeldAnswers(SERVICES);
  const api = createFakeApi({ getServices: (_query, signal) => services.answer(signal) });
  const rangeButton = page.getByRole("button", { name: /^Range:/ });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/services");
    await expect.element(page.getByRole("row", { name: /api/ })).toBeInTheDocument();

    await userEvent.click(page.getByRole("button", { name: /^Earlier by/ }));
    await expect.poll(() => readShownRange(rangeButton.element())).not.toBe("Last hour");
    const firstShift = readShownRange(rangeButton.element());
    await userEvent.click(page.getByRole("button", { name: /^Earlier by/ }));
    await expect.poll(() => readShownRange(rangeButton.element())).not.toBe(firstShift);
    expect(readShownRange(rangeButton.element())).toBe(readNamedRange(rangeButton.element()));

    services.releaseAll(SERVICES);
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("leaving the services while a range change loads keeps the next pages navigating", async () => {
  const services = createHeldAnswers(SERVICES);
  const api = createFakeApi({
    getServices: (_query, signal) => services.answer(signal),
    listDashboards: () => Promise.resolve(toDashboardList(["Latency"])),
  });

  const { artifact } = await captureArtifact(async () => {
    const app = mountApp(api, "/services");
    await expect.element(page.getByRole("row", { name: /api/ })).toBeInTheDocument();
    await userEvent.click(page.getByRole("button", { name: /^Earlier by/ }));

    await expectNavigationToReach(app, "Dashboards", "/dashboards");
    services.releaseAll(SERVICES);
    await expectNavigationToReach(app, "Services", "/services");
    await expect.element(page.getByRole("row", { name: /api/ })).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

function readShownRange(button: Element): string {
  return button.textContent.replaceAll(/\s/g, "");
}

function readNamedRange(button: Element): string {
  return (button.getAttribute("aria-label") ?? "").replace("Range:", "").replaceAll(/\s/g, "");
}
