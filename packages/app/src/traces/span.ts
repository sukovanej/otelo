// How the traces pages name the fields of a span.

import { SPAN_KINDS, SPAN_STATUSES, type Span } from "../api";
import { attributes, builtin, resource, type Section } from "../FieldTable";

export const kindName = (kind: number) => SPAN_KINDS[kind] ?? String(kind);
export const statusName = (status: number) => SPAN_STATUSES[status] ?? String(status);

/** The fields of a span, with the literals the query language compares
 * them to. */
export function spanSections(span: Span): Section[] {
  const kind = kindName(span.kind);
  const status = statusName(span.status);
  return [
    {
      title: "Span",
      fields: [
        builtin("service", span.service),
        builtin("name", span.name),
        // The query language has no name for an unspecified kind.
        builtin("kind", kind, span.kind === 0 ? null : kind),
        builtin("status", status, status),
        builtin("trace_id", span.trace_id),
        builtin("span_id", span.span_id),
        { ...builtin("parent_span_id", span.parent_span_id), name: undefined },
      ].filter((field) => field.value !== null),
    },
    { title: "Attributes", fields: attributes(span.attributes) },
    { title: "Resource", fields: resource(span.resource) },
  ];
}
