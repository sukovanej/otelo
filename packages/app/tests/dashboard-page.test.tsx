import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import type { Api, Dashboard, DashboardDefinition, MetricSeries } from "@otelo/api";
import {
  createDeferred,
  createHeldAnswers,
  expectNoReactivityMistakes,
  type HeldAnswers,
} from "@otelo/testing";

import { createFakeApi } from "./fake-api";
import { toDashboardList, toEmptyMetricSeries, toMetricList } from "./fixtures";
import { mountApp } from "./mount";
import { expectNavigationToReach } from "./navigation";

const SAVED_DEFINITION: DashboardDefinition = {
  name: "Latency",
  description: "",
  widgets: [],
};

const CPU_DEFINITION: DashboardDefinition = {
  name: "Hosts",
  description: "",
  widgets: [
    {
      title: "CPU",
      layout: { column: 0, row: 0, width: 6, height: 6 },
      display: {
        kind: "timeseries",
        chart: "line",
        queries: [
          { by: [], query: { signal: "metrics", name: "cpu", filter: "", aggregation: "avg" } },
          { by: [], query: { signal: "metrics", name: "memory", filter: "", aggregation: "avg" } },
        ],
      },
    },
  ],
};

const VALUE_DEFINITION: DashboardDefinition = {
  name: "Hosts",
  description: "",
  widgets: [
    {
      title: "CPU",
      layout: { column: 0, row: 0, width: 6, height: 6 },
      display: {
        kind: "value",
        query: { signal: "metrics", name: "cpu", filter: "", aggregation: "avg" },
      },
    },
  ],
};

const CPU_SERIES = toEmptyMetricSeries("cpu");

const MEMORY_SERIES = toEmptyMetricSeries("memory");

test("a dashboard keeps its edits apart from what it saved, through reloads, a discard and a save", async () => {
  const sentDefinitions: DashboardDefinition[] = [];
  let storedDefinition = SAVED_DEFINITION;
  const api = createFakeApi({
    getDashboard: () => Promise.resolve(toDashboard(storedDefinition)),
    replaceDashboard: (_id, definition) => {
      storedDefinition = definition;
      sentDefinitions.push(definition);
      return Promise.resolve(toDashboard(definition));
    },
  });
  const nameInput = page.getByLabelText("Name");
  const unsavedChanges = page.getByText("Unsaved changes");

  const { artifact } = await captureArtifact(async () => {
    const { queryClient } = mountApp(api, "/dashboards/1");
    await expect.element(nameInput).toHaveValue("Latency");

    await queryClient.refetchQueries();
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();

    await userEvent.fill(nameInput, "Latency by route");
    await expect.element(unsavedChanges).toBeInTheDocument();
    await queryClient.refetchQueries();
    await expect.element(page.getByText("Loading…")).not.toBeInTheDocument();
    await expect.element(nameInput).toHaveValue("Latency by route");
    await userEvent.click(page.getByRole("button", { name: "Discard", exact: true }));
    await expect.element(unsavedChanges).not.toBeInTheDocument();
    await expect.element(nameInput).toHaveValue("Latency");

    await userEvent.fill(nameInput, "Latency by route");
    await expect.element(unsavedChanges).toBeInTheDocument();
    await userEvent.click(page.getByRole("button", { name: "Save", exact: true }));
    await expect.element(unsavedChanges).not.toBeInTheDocument();
    await expect.element(nameInput).toHaveValue("Latency by route");
  });

  expect(sentDefinitions).toEqual([{ ...SAVED_DEFINITION, name: "Latency by route" }]);
  expectNoReactivityMistakes(artifact);
});

test("a widget draws a skeleton until its first answer lands", async () => {
  const series = createDeferred<MetricSeries>();
  const api = createFakeApi({
    getDashboard: () => Promise.resolve(toDashboard(VALUE_DEFINITION)),
    getMetricSeries: () => series.promise,
  });
  const skeleton = page.getByRole("status", { name: "Loading" });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/dashboards/1");
    await expect.element(skeleton).toBeInTheDocument();
    await expect.element(page.getByRole("heading", { name: "CPU" })).toBeInTheDocument();

    series.resolve(CPU_SERIES);
    await expect.element(skeleton).not.toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("opening a widget's editor requests none of the series the widget shows", async () => {
  const series = createHeldSeries();
  const api = createCpuDashboardApi(series);

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/dashboards/1");
    await openCpuEditor();
    await expect.element(page.getByRole("tab", { name: "Query A" })).toBeInTheDocument();
  });

  expect([...series.cpu.held, ...series.memory.held]).toHaveLength(0);
  expectNoReactivityMistakes(artifact);
});

test("an added query shows its own form while the preview loads", async () => {
  const series = createHeldSeries();
  const api = createCpuDashboardApi(series);
  const metricField = page.getByRole("combobox", { name: "Metric" });

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/dashboards/1");
    await openCpuEditor();
    await expect.element(metricField).toHaveValue("cpu");

    await userEvent.click(page.getByRole("button", { name: "Add a query" }));
    await expect
      .element(page.getByRole("tab", { name: "Query C" }))
      .toHaveAttribute("aria-selected", "true");
    await expect.element(metricField).toHaveValue("memory");

    releaseHeldSeries(series);
    await expect.element(metricField).toHaveValue("memory");
  });

  expectNoReactivityMistakes(artifact);
});

test("cancelling an edit while its preview loads keeps the next pages navigating", async () => {
  const series = createHeldSeries();
  const api = createCpuDashboardApi(series);

  const { artifact } = await captureArtifact(async () => {
    const app = mountApp(api, "/dashboards/1");
    await openCpuEditor();
    await userEvent.click(page.getByRole("button", { name: "Add a query" }));
    await expect
      .element(page.getByRole("tab", { name: "Query C" }))
      .toHaveAttribute("aria-selected", "true");
    await userEvent.click(page.getByRole("button", { name: "Cancel", exact: true }));
    await expect.element(page.getByRole("dialog")).not.toBeInTheDocument();

    releaseHeldSeries(series);
    await expectNavigationToReach(app, "Dashboards", "/dashboards");
    await expect.element(page.getByRole("row", { name: /Hosts/ })).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});

test("switching a value widget to a time series shows the chart form while the preview loads", async () => {
  const series = createHeldSeries();
  const api = createCpuDashboardApi(series, VALUE_DEFINITION);

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/dashboards/1");
    await openCpuEditor();
    await userEvent.click(page.getByRole("tab", { name: "Time series" }));
    await expect.element(page.getByRole("tab", { name: "Line" })).toBeInTheDocument();
    await expect.element(page.getByRole("status", { name: "Loading" })).toBeInTheDocument();
    await userEvent.click(page.getByRole("button", { name: "Cancel", exact: true }));
    await expect.element(page.getByRole("dialog")).not.toBeInTheDocument();

    releaseHeldSeries(series);
  });

  expectNoReactivityMistakes(artifact);
});

test("switching a query to spans shows the span form while the preview loads", async () => {
  const series = createHeldSeries();
  const api = createCpuDashboardApi(series);

  const { artifact } = await captureArtifact(async () => {
    mountApp(api, "/dashboards/1");
    await openCpuEditor();
    await userEvent.click(page.getByRole("tab", { name: "Spans" }));
    await expect.element(page.getByRole("button", { name: /^Measure/ })).toBeInTheDocument();
    await expect.element(page.getByRole("status", { name: "Loading" })).toBeInTheDocument();
    await userEvent.click(page.getByRole("button", { name: "Cancel", exact: true }));
    await expect.element(page.getByRole("dialog")).not.toBeInTheDocument();

    releaseHeldSeries(series);
  });

  expectNoReactivityMistakes(artifact);
});

interface HeldSeries {
  readonly cpu: HeldAnswers<MetricSeries>;
  readonly memory: HeldAnswers<MetricSeries>;
}

function createHeldSeries(): HeldSeries {
  return { cpu: createHeldAnswers(CPU_SERIES), memory: createHeldAnswers(MEMORY_SERIES) };
}

function releaseHeldSeries(series: HeldSeries) {
  series.cpu.releaseAll(CPU_SERIES);
  series.memory.releaseAll(MEMORY_SERIES);
}

function createCpuDashboardApi(
  series: HeldSeries,
  definition: DashboardDefinition = CPU_DEFINITION,
): Api {
  return createFakeApi({
    getDashboard: () => Promise.resolve(toDashboard(definition)),
    getMetricSeries: (name, _query, signal) =>
      (name === "memory" ? series.memory : series.cpu).answer(signal),
    getMetrics: () => Promise.resolve(toMetricList(["cpu", "memory"])),
    getAttributeKeys: () => Promise.resolve({ record: [], resource: [] }),
    listDashboards: () => Promise.resolve(toDashboardList(["Hosts"])),
  });
}

async function openCpuEditor() {
  await userEvent.hover(page.getByRole("heading", { name: "CPU" }));
  await userEvent.click(page.getByRole("button", { name: "Edit the widget" }));
  await expect.element(page.getByRole("dialog")).toBeInTheDocument();
}

function toDashboard(definition: DashboardDefinition): Dashboard {
  return {
    id: 1,
    created_at: "2026-10-01T00:00:00Z",
    updated_at: "2026-10-01T00:00:00Z",
    definition,
  };
}
