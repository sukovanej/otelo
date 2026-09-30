import { expect, test } from "vitest";

import type { TraceSpan } from "@otelo/api";

import {
  buildSpanTree,
  dropCollapsedRows,
  measureTraceNanos,
  type TreeRow,
} from "../src/traces/tree";

test("buildSpanTree puts each span under its parent, depth first", () => {
  const rows = buildSpanTree([
    makeSpan("root", null, 0, 100),
    makeSpan("a", "root", 10, 30),
    makeSpan("b", "root", 50, 40),
    makeSpan("a1", "a", 12, 5),
  ]);
  expect(printTree(rows)).toEqual(["root +0", "  a +10", "    a1 +12", "  b +50"]);
  expect(rows.map((row) => row.childCount)).toEqual([2, 1, 0, 0]);
  expect(measureTraceNanos(rows)).toBe(100e6);
});

test("buildSpanTree makes a span with a missing parent a root", () => {
  const rows = buildSpanTree([
    makeSpan("orphan", "gone", 5, 10),
    makeSpan("child", "orphan", 6, 1),
  ]);
  expect(printTree(rows)).toEqual(["orphan +0", "  child +1"]);
});

test("buildSpanTree keeps spans that are each other's parents", () => {
  const rows = buildSpanTree([makeSpan("x", "y", 0, 1), makeSpan("y", "x", 1, 1)]);
  expect(printTree(rows)).toEqual(["x +0", "  y +1"]);
});

test("dropCollapsedRows hides the descendants of a folded span", () => {
  const rows = buildSpanTree([
    makeSpan("root", null, 0, 100),
    makeSpan("a", "root", 10, 30),
    makeSpan("a1", "a", 12, 5),
    makeSpan("b", "root", 50, 40),
  ]);
  const ids = (folded: string[]) =>
    dropCollapsedRows(rows, new Set(folded)).map((row) => row.span.span_id);
  expect(ids([])).toEqual(["root", "a", "a1", "b"]);
  expect(ids(["a"])).toEqual(["root", "a", "b"]);
  expect(ids(["root"])).toEqual(["root"]);
});

function makeSpan(
  id: string,
  parent: string | null,
  startMs: number,
  durationMs: number,
): TraceSpan {
  return {
    trace_id: "0af7651916cd43dd8448eb211c80319c",
    span_id: id,
    parent_span_id: parent,
    service: "api",
    name: id,
    kind: 2,
    started_at: new Date(Date.UTC(2026, 8, 28, 10, 0, 0, startMs)).toISOString(),
    duration_ns: durationMs * 1e6,
    status: 0,
    attributes: {},
    events: [],
    resource: {},
  };
}

function printTree(rows: ReadonlyArray<TreeRow>): string[] {
  return rows.map(
    (row) => `${"  ".repeat(row.depth)}${row.span.span_id} +${row.startOffsetNanos / 1e6}`,
  );
}
