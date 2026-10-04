import { expect, test } from "vitest";

import type { GroupedQuery, WidgetDisplay } from "@otelo/api";

import {
  changeDisplayKind,
  changeQuerySignal,
  describeWidgetQuery,
  listCatalogGroupingOptions,
  describeWidgetTitle,
} from "../src/dashboards/widget";

const P95_BY_SERVICE: GroupedQuery = {
  query: { signal: "spans", filter: "kind = server", measure: "p95" },
  by: ["service"],
};

const LATENCY: WidgetDisplay = {
  kind: "timeseries",
  chart: "area",
  queries: [P95_BY_SERVICE, { query: { signal: "logs", filter: "" }, by: [] }],
};

test("a widget without a title is named after its first query", () => {
  expect(
    describeWidgetTitle({
      title: "",
      layout: { column: 0, row: 0, width: 6, height: 6 },
      display: LATENCY,
    }),
  ).toBe("P95 of spans where kind = server");
  expect(
    describeWidgetTitle({
      title: "Latency",
      layout: { column: 0, row: 0, width: 6, height: 6 },
      display: LATENCY,
    }),
  ).toBe("Latency");
  expect(
    describeWidgetQuery({ signal: "metrics", name: "", filter: "", aggregation: "rate" }),
  ).toBe("Rate of a metric");
});

test("a widget keeps its first query when it changes what it draws", () => {
  expect(changeDisplayKind(LATENCY, "value")).toEqual({
    kind: "value",
    query: { signal: "spans", filter: "kind = server", measure: "p95" },
  });
  expect(changeDisplayKind(LATENCY, "toplist")).toEqual({
    kind: "toplist",
    query: P95_BY_SERVICE,
    limit: 10,
    order: "highest",
  });
  const value = changeDisplayKind(LATENCY, "value");
  expect(changeDisplayKind(value, "toplist")).toMatchObject({ query: { by: ["service"] } });
  expect(changeDisplayKind(value, "timeseries")).toMatchObject({ chart: "line" });
  expect(changeDisplayKind(LATENCY, "timeseries")).toBe(LATENCY);
});

test("a query of another signal starts over", () => {
  const spans = { signal: "spans", filter: "kind = server", measure: "count" } as const;
  expect(changeQuerySignal(spans, "spans")).toBe(spans);
  expect(changeQuerySignal(spans, "metrics")).toEqual({
    signal: "metrics",
    name: "",
    filter: "",
    aggregation: "avg",
  });
});

test("logs group by their fields, attributes, resource keys, and names chosen before", () => {
  const options = listCatalogGroupingOptions(
    "logs",
    {
      record: [
        { key: "user.id", count: 3, indexed: false, type: "int" },
        { key: "with space", count: 1, indexed: false, type: "string" },
      ],
      resource: [{ key: "host.name", count: 1, indexed: false, type: "string" }],
    },
    ["tenant", "resource.region"],
  );
  expect(options.map((option) => [option.section, option.value])).toEqual([
    ["Fields", "service"],
    ["Fields", "level"],
    ["Attributes", "user.id"],
    ["Attributes", "tenant"],
    ["Resource", "resource.host.name"],
    ["Resource", "resource.region"],
  ]);
});
