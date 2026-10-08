import { captureArtifact } from "@solidjs/diagnostics";
import { createSignal } from "solid-js";
import { expect, test } from "vitest";
import { page } from "vitest/browser";

import type { Widget } from "@otelo/api";
import { expectNoReactivityMistakes } from "@otelo/testing";

import DashboardWidget from "../src/dashboards/DashboardWidget";
import { createFakeApi } from "./fake-api";
import { mountWithApi } from "./mount";

const UNNAMED_METRIC_VALUE: Widget = {
  title: "",
  layout: { column: 0, row: 0, width: 6, height: 6 },
  display: {
    kind: "value",
    query: { signal: "metrics", name: "", filter: "", aggregation: "avg" },
  },
};

const NAMED_METRIC_VALUE: Widget = {
  ...UNNAMED_METRIC_VALUE,
  display: {
    kind: "value",
    query: { signal: "metrics", name: "cpu", filter: "", aggregation: "avg" },
  },
};

const RANGE = {
  since: () => "now-1h",
  until: () => "",
  live: () => false,
  setRange: () => {},
};

test("a widget redraws its message when the reason for no data changes", async () => {
  const api = createFakeApi({
    getMetricSeries: (name) =>
      Promise.resolve({
        name,
        start_at: "2026-10-01T00:00:00Z",
        end_at: "2026-10-01T01:00:00Z",
        step_ns: 60e9,
        resolution: "raw",
        groups: [],
        truncated: false,
      }),
  });
  const [widget, setWidget] = createSignal(UNNAMED_METRIC_VALUE);

  const { artifact } = await captureArtifact(async () => {
    mountWithApi(api, () => <DashboardWidget widget={widget()} range={RANGE} />);
    await expect.element(page.getByText("Pick a metric to see its numbers.")).toBeInTheDocument();
    setWidget(NAMED_METRIC_VALUE);
    await expect.element(page.getByText("No series match.")).toBeInTheDocument();
  });

  expectNoReactivityMistakes(artifact);
});
