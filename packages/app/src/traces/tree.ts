// The spans of one trace as a tree, laid out on the time of the trace.

import type { TraceSpan } from "@siner/api";

import { nanosBetween } from "../time";

export interface TreeRow {
  span: TraceSpan;
  /** How many ancestors the span has in the trace. */
  depth: number;
  /** How many children it has. */
  children: number;
  /** The nanoseconds from the start of the trace to the start of the span. */
  offset: number;
}

/**
 * The spans depth first, each under its parent, and siblings in the order
 * of `spans`, which the API sorts by start. A span whose parent is not in
 * the list is a root.
 */
export function spanTree(spans: TraceSpan[]): TreeRow[] {
  const start = spans.reduce<string | undefined>(
    (first, span) =>
      first === undefined || nanosBetween(first, span.time) < 0 ? span.time : first,
    undefined,
  );
  const ids = new Set(spans.map((span) => span.span_id));
  const children = new Map<string | undefined, TraceSpan[]>();
  for (const span of spans) {
    const parent =
      span.parent_span_id !== null && ids.has(span.parent_span_id)
        ? span.parent_span_id
        : undefined;
    const siblings = children.get(parent);
    if (siblings) siblings.push(span);
    else children.set(parent, [span]);
  }

  const rows: TreeRow[] = [];
  const seen = new Set<TraceSpan>();
  const visit = (span: TraceSpan, depth: number) => {
    if (seen.has(span)) return;
    seen.add(span);
    const kids = children.get(span.span_id) ?? [];
    rows.push({
      span,
      depth,
      children: kids.length,
      offset: start === undefined ? 0 : nanosBetween(start, span.time),
    });
    for (const kid of kids) visit(kid, depth + 1);
  };
  for (const root of children.get(undefined) ?? []) visit(root, 0);
  // Spans that are each other's ancestors have no root; they go last.
  for (const span of spans) visit(span, 0);
  return rows;
}

/** The rows without the descendants of the spans in `collapsed`. */
export function visibleRows(rows: TreeRow[], collapsed: ReadonlySet<string>): TreeRow[] {
  const shown: TreeRow[] = [];
  let hiddenBelow: number | undefined;
  for (const row of rows) {
    if (hiddenBelow !== undefined && row.depth > hiddenBelow) continue;
    hiddenBelow = collapsed.has(row.span.span_id) ? row.depth : undefined;
    shown.push(row);
  }
  return shown;
}

/** The nanoseconds from the start of the first span to the end of the last. */
export const traceLength = (rows: TreeRow[]) =>
  Math.max(0, ...rows.map((row) => row.offset + row.span.duration_ns));
