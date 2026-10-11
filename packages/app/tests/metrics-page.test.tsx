import { captureArtifact } from "@solidjs/diagnostics";
import { type GraphSize, graphSize } from "solid-js/attribution";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import type { MetricList, MetricSeries } from "@otelo/api";
import { createHeldAnswers, expectNoReactivityMistakes } from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toBusyMetricSeries, toDashboardList, toEmptyMetricSeries, toTraces } from "./fixtures";
import { mountApp, type MountedApp } from "./mount";
import { expectNavigationToReach } from "./navigation";
import { switchToPhoneScreen } from "./screen";

const BUSY_GROUP_COUNT = 10;

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

test("on a phone the metric names and the open metric take turns", async () => {
  await switchToPhoneScreen();
  const api = createFakeApi({
    getMetrics: () => Promise.resolve(ROUTED_METRICS),
    getMetricSeries: () => Promise.resolve(CPU_SERIES),
  });
  const names = page.getByRole("navigation", { name: "Metrics" });

  const { artifact } = await captureArtifact(async () => {
    const { history } = mountApp(api, "/metrics");
    await userEvent.click(names.getByRole("link", { name: /process\.cpu/ }));
    await expect.element(page.getByText(/Steps of/)).toBeVisible();
    await expect
      .element(page.getByRole("navigation", { name: "Metrics", includeHidden: true }))
      .not.toBeVisible();

    await userEvent.click(page.getByRole("link", { name: "← Every metric" }));
    await expect.poll(() => history.get()).toBe("/metrics");
    await expect.element(names.getByRole("link", { name: /process\.cpu/ })).toBeVisible();
  });

  expectNoReactivityMistakes(artifact);
});

test("a chart of many points tracks the answer, not each point", async () => {
  const api = createFakeApi({
    getMetrics: () => Promise.resolve(ROUTED_METRICS),
    getMetricSeries: (name) => Promise.resolve(toBusyMetricSeries(name, BUSY_GROUP_COUNT)),
  });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/metrics/process.cpu");
    await expect.element(page.getByText(/Steps of/)).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("a metric page keeps no signal per value of its points, however often it is visited", async () => {
  let seriesAnswerCount = 0;
  const api = createFakeApi({
    getTraces: () => Promise.resolve(toTraces([])),
    getMetrics: () => Promise.resolve(ROUTED_METRICS),
    getMetricSeries: (name) => {
      seriesAnswerCount += 1;
      return Promise.resolve(toBusyMetricSeries(name, BUSY_GROUP_COUNT, seriesAnswerCount));
    },
  });
  const pointCount = countPoints(toBusyMetricSeries("process.cpu", BUSY_GROUP_COUNT));

  const { artifact } = await captureArtifact(async () => {
    const app = mountApp(api, "/traces");
    await visitMetricPage(app);
    const secondVisit = await visitMetricPage(app);
    const thirdVisit = await visitMetricPage(app);
    expect(thirdVisit).toEqual(secondVisit);
    expect(thirdVisit.signals).toBeLessThan(2 * pointCount);
  });

  expectNoReactivityMistakes(artifact);
});

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

async function visitMetricPage(app: MountedApp): Promise<GraphSize> {
  await expectNavigationToReach(app, "Metrics", "/metrics");
  await userEvent.click(page.getByRole("link", { name: /process\.cpu/ }));
  await expect.element(page.getByText(/Steps of/)).toBeInTheDocument();
  await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
  const graphOfMetricPage = graphSize();
  await expectNavigationToReach(app, "Traces", "/traces");
  return graphOfMetricPage;
}

function countPoints(series: MetricSeries): number {
  return series.groups.reduce((count, group) => count + group.buckets.length, 0);
}
