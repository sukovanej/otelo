import type { SpanGroup } from "@otelo/api";

import { addTerm, quoteString, writeLiteral } from "../query";

export const SPAN_GROUPINGS: Record<SpanGroupingKind, SpanGrouping> = {
  route: {
    kind: "route",
    by: ["http.request.method", "http.route"],
    writeFilter: (service) =>
      `service = ${quoteString(service)} kind = server has(http.request.method)`,
  },
  query: {
    kind: "query",
    by: ["db.system.name", "db.namespace", "db.query.text"],
    writeFilter: (service) => `service = ${quoteString(service)} has(db.system.name)`,
  },
};

export type SpanGroupingKind = "route" | "query";

export interface SpanGrouping {
  readonly kind: SpanGroupingKind;
  readonly by: ReadonlyArray<string>;
  readonly writeFilter: (service: string) => string;
}

export function writeGroupFilter(
  filter: string,
  by: ReadonlyArray<string>,
  group: SpanGroup,
): string {
  return by.reduce((query, field) => {
    const value = group.values[field];
    if (value === undefined) return addTerm(query, `NOT has(${field})`);
    const literal = writeLiteral(value);
    return literal === undefined ? query : addTerm(query, `${field} = ${literal}`);
  }, filter);
}

export function isSpanGroupingKind(text: string | undefined): text is SpanGroupingKind {
  return text !== undefined && Object.hasOwn(SPAN_GROUPINGS, text);
}
