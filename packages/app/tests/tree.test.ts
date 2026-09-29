import { expect, test } from "vitest";

import type { TraceSpan } from "@otelo/api";

import { spanTree, traceLength, visibleRows } from "../src/traces/tree";

/** A span that starts `ms` milliseconds into the trace. */
const span = (id: string, parent: string | null, ms: number, durationMs: number): TraceSpan => ({
  trace_id: "0af7651916cd43dd8448eb211c80319c",
  span_id: id,
  parent_span_id: parent,
  service: "api",
  name: id,
  kind: 2,
  started_at: new Date(Date.UTC(2026, 8, 28, 10, 0, 0, ms)).toISOString(),
  duration_ns: durationMs * 1e6,
  status: 0,
  attributes: {},
  events: [],
  resource: {},
});

const layout = (rows: ReturnType<typeof spanTree>) =>
  rows.map((row) => `${"  ".repeat(row.depth)}${row.span.span_id} +${row.offset / 1e6}`);

test("spanTree puts each span under its parent, depth first", () => {
  const rows = spanTree([
    span("root", null, 0, 100),
    span("a", "root", 10, 30),
    span("b", "root", 50, 40),
    span("a1", "a", 12, 5),
  ]);
  expect(layout(rows)).toEqual(["root +0", "  a +10", "    a1 +12", "  b +50"]);
  expect(rows.map((row) => row.children)).toEqual([2, 1, 0, 0]);
  expect(traceLength(rows)).toBe(100e6);
});

test("spanTree makes a span with a missing parent a root", () => {
  const rows = spanTree([span("orphan", "gone", 5, 10), span("child", "orphan", 6, 1)]);
  expect(layout(rows)).toEqual(["orphan +0", "  child +1"]);
});

test("spanTree keeps spans that are each other's parents", () => {
  const rows = spanTree([span("x", "y", 0, 1), span("y", "x", 1, 1)]);
  expect(layout(rows)).toEqual(["x +0", "  y +1"]);
});

test("visibleRows hides the descendants of a folded span", () => {
  const rows = spanTree([
    span("root", null, 0, 100),
    span("a", "root", 10, 30),
    span("a1", "a", 12, 5),
    span("b", "root", 50, 40),
  ]);
  const ids = (folded: string[]) =>
    visibleRows(rows, new Set(folded)).map((row) => row.span.span_id);
  expect(ids([])).toEqual(["root", "a", "a1", "b"]);
  expect(ids(["a"])).toEqual(["root", "a", "b"]);
  expect(ids(["root"])).toEqual(["root"]);
});
