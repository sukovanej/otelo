import type { TraceSpan } from "@otelo/api";

import { measureNanosBetween } from "../time";

export interface TreeRow {
  readonly span: TraceSpan;
  readonly depth: number;
  readonly childCount: number;
  readonly startOffsetNanos: number;
}

// Siblings keep the order of `spans`, which the API sorts by start.
export function buildSpanTree(spans: ReadonlyArray<TraceSpan>): TreeRow[] {
  const traceStart = spans.reduce<string | undefined>(
    (earliest, span) =>
      earliest === undefined || measureNanosBetween(earliest, span.started_at) < 0
        ? span.started_at
        : earliest,
    undefined,
  );
  const spanIds = new Set(spans.map((span) => span.span_id));
  const childrenByParentId = new Map<string | undefined, TraceSpan[]>();
  for (const span of spans) {
    const parentId =
      span.parent_span_id !== null && spanIds.has(span.parent_span_id)
        ? span.parent_span_id
        : undefined;
    const siblings = childrenByParentId.get(parentId);
    if (siblings) siblings.push(span);
    else childrenByParentId.set(parentId, [span]);
  }

  const rows: TreeRow[] = [];
  const visitedSpans = new Set<TraceSpan>();
  const visitSpan = (span: TraceSpan, depth: number) => {
    if (visitedSpans.has(span)) return;
    visitedSpans.add(span);
    const childSpans = childrenByParentId.get(span.span_id) ?? [];
    rows.push({
      span,
      depth,
      childCount: childSpans.length,
      startOffsetNanos:
        traceStart === undefined ? 0 : measureNanosBetween(traceStart, span.started_at),
    });
    for (const child of childSpans) visitSpan(child, depth + 1);
  };
  for (const root of childrenByParentId.get(undefined) ?? []) visitSpan(root, 0);
  // Spans that are each other's ancestors have no root; they go last.
  for (const span of spans) visitSpan(span, 0);
  return rows;
}

export function dropCollapsedRows(
  rows: ReadonlyArray<TreeRow>,
  collapsedSpanIds: ReadonlySet<string>,
): TreeRow[] {
  const shownRows: TreeRow[] = [];
  let collapsedDepth: number | undefined;
  for (const row of rows) {
    if (collapsedDepth !== undefined && row.depth > collapsedDepth) continue;
    collapsedDepth = collapsedSpanIds.has(row.span.span_id) ? row.depth : undefined;
    shownRows.push(row);
  }
  return shownRows;
}

export function measureTraceNanos(rows: ReadonlyArray<TreeRow>): number {
  return Math.max(0, ...rows.map((row) => row.startOffsetNanos + row.span.duration_ns));
}
