import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import type { MetricList } from "@otelo/api";
import { createHeldAnswers, expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toDashboardList, toEmptyMetricSeries } from "./fixtures";
import { mountApp } from "./mount";
import { expectNavigationToReach } from "./navigation";

const CPU_SERIES = toEmptyMetricSeries("process.cpu");

const ROUTED_METRICS: MetricList = {
  series: [
    {
      kind: "gauge",
      name: "process.cpu",
      service: "api",
      unit: "By",
      attributes: { "http.route": "/orders" },
      resource: {},
    },
  ],
  truncated: false,
};

test("the group by list leaves out a picked name while the series load", async () => {
  const series = createHeldAnswers(CPU_SERIES);
  const api = createFakeApi({
    getMetrics: () => Promise.resolve(ROUTED_METRICS),
    getMetricSeries: (_name, _query, signal) => series.answer(signal),
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/metrics/process.cpu");
    await expect.element(page.getByText(/Steps of/)).toBeInTheDocument();

    await userEvent.click(page.getByRole("combobox", { name: "Group by" }));
    await userEvent.click(page.getByRole("option", { name: "http.route" }));
    await expect.element(page.getByRole("option", { name: "http.route" })).not.toBeInTheDocument();
    await expect.element(page.getByRole("option", { name: "service" })).toBeInTheDocument();

    series.releaseAll(CPU_SERIES);
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("the top list marks the picked count while the series load", async () => {
  const series = createHeldAnswers(CPU_SERIES);
  const api = createFakeApi({
    getMetrics: () => Promise.resolve(ROUTED_METRICS),
    getMetricSeries: (_name, _query, signal) => series.answer(signal),
  });
  const topButton = page.getByRole("button", { name: /^Top/ });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/metrics/process.cpu");
    await expect.element(page.getByText(/Steps of/)).toBeInTheDocument();

    await userEvent.click(topButton);
    await userEvent.click(page.getByRole("option", { name: "5" }));
    await userEvent.click(topButton);
    await expect
      .element(page.getByRole("option", { name: "5" }))
      .toHaveAttribute("aria-selected", "true");
    await expect
      .element(page.getByRole("option", { name: "Every group" }))
      .toHaveAttribute("aria-selected", "false");

    series.releaseAll(CPU_SERIES);
    await userEvent.keyboard("{Escape}");
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("leaving a metric while its top count loads keeps the next pages navigating", async () => {
  const series = createHeldAnswers(CPU_SERIES);
  const api = createFakeApi({
    getMetrics: () => Promise.resolve(ROUTED_METRICS),
    getMetricSeries: (_name, _query, signal) => series.answer(signal),
    listDashboards: () => Promise.resolve(toDashboardList(["Latency"])),
  });

  const { artifact } = await captureArtifact(async () => {
    const app = mountApp(api, "/metrics/process.cpu");
    await expect.element(page.getByText(/Steps of/)).toBeInTheDocument();
    await userEvent.click(page.getByRole("button", { name: /^Top/ }));
    await userEvent.click(page.getByRole("option", { name: "5" }));

    await expectNavigationToReach(app, "Dashboards", "/dashboards");
    series.releaseAll(CPU_SERIES);
    await expectNavigationToReach(app, "Metrics", "/metrics");
  });

  expectNoReactivityMistakes(artifact);
});
