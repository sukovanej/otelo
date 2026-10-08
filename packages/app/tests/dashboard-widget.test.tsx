import { captureArtifact } from "@solidjs/diagnostics";
import { render } from "@solidjs/web";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { createSignal } from "solid-js";
import { afterEach, expect, test, vi } from "vitest";

import type { Widget } from "@otelo/api";

import DashboardWidget from "../src/dashboards/DashboardWidget";
import { expectNoReactivityMistakes } from "./diagnostics";

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

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.replaceChildren();
});

test("a widget redraws its message when the reason for no data changes", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json({ groups: [] })),
  );
  const container = document.body.appendChild(document.createElement("div"));
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const [widget, setWidget] = createSignal(UNNAMED_METRIC_VALUE);

  const { artifact } = await captureArtifact(async () => {
    render(
      () => (
        <QueryClientProvider client={queryClient}>
          <DashboardWidget widget={widget()} range={RANGE} />
        </QueryClientProvider>
      ),
      container,
    );
    await vi.waitFor(() =>
      expect(container.textContent).toContain("Pick a metric to see its numbers."),
    );
    setWidget(NAMED_METRIC_VALUE);
    await vi.waitFor(() => expect(container.textContent).toContain("No series match."));
  });

  expectNoReactivityMistakes(artifact);
});
