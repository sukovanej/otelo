import { captureArtifact } from "@solidjs/diagnostics";
import { createSignal } from "solid-js";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import type { GroupedQuery } from "@otelo/api";
import { expectNoReactivityMistakes } from "@otelo/testing";

import WidgetEditorQueries from "../src/dashboards/WidgetEditor/widget-editor-queries";
import { createFakeApi } from "./fake-api";
import { mountWithApi } from "./mount";

const RANGE = {
  since: () => "now-1h",
  until: () => "",
  live: () => false,
};

const QUERIES: ReadonlyArray<GroupedQuery> = [
  { query: { signal: "logs", filter: "level >= warn" }, by: [] },
  { query: { signal: "logs", filter: "level >= error" }, by: [] },
  { query: { signal: "logs", filter: "service = api" }, by: [] },
];

test("removing a query after the shown one keeps the shown one", async () => {
  const api = createFakeApi({
    getAttributeKeys: () => Promise.resolve({ record: [], resource: [] }),
  });
  const [queries, setQueries] = createSignal(QUERIES);

  const { artifact } = await captureArtifact(async () => {
    mountWithApi(api, () => (
      <WidgetEditorQueries queries={queries()} range={RANGE} onChange={setQueries} />
    ));
    await userEvent.click(page.getByRole("tab", { name: "Query B" }));
    await expect.element(page.getByRole("tabpanel", { name: "Query B" })).toBeInTheDocument();

    await userEvent.click(page.getByRole("button", { name: "Remove query C" }));

    await expect.element(page.getByRole("tab", { name: "Query C" })).not.toBeInTheDocument();
    await expect
      .element(page.getByRole("tab", { name: "Query B" }))
      .toHaveAttribute("aria-selected", "true");
    await expect.element(page.getByRole("tabpanel", { name: "Query B" })).toBeInTheDocument();
  });

  expect(queries()).toEqual(QUERIES.slice(0, 2));
  expectNoReactivityMistakes(artifact);
});
