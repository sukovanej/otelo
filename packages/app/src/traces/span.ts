import type { TraceSpan } from "@otelo/api";

import {
  type Field,
  type FieldSection,
  listAttributeFields,
  linkField,
  listResourceFields,
  toBuiltinField,
  toUnnamedField,
} from "../field";
import { toServicePagePath, toTracePagePath } from "../path";

// As the query language writes them.
const SPAN_KIND_NAMES = ["unspecified", "internal", "server", "client", "producer", "consumer"];
const SPAN_STATUS_NAMES = ["unset", "ok", "error"];

const ERROR_STATUS = 2;

export function toKindName(kind: number): string {
  return SPAN_KIND_NAMES[kind] ?? String(kind);
}

export function isFailedSpan(span: TraceSpan): boolean {
  return span.status === ERROR_STATUS;
}

export function listSpanSections(span: TraceSpan): FieldSection[] {
  const kind = toKindName(span.kind);
  const status = toStatusName(span.status);
  const spanFields: Field[] = [
    linkField(toBuiltinField("service", span.service), toServicePagePath(span.service)),
    toBuiltinField("name", span.name),
    // The query language has no name for an unspecified kind.
    toBuiltinField("kind", kind, span.kind === 0 ? null : kind),
    toBuiltinField("status", status, status),
    linkField(toBuiltinField("trace_id", span.trace_id), toTracePagePath(span.trace_id)),
    linkField(
      toBuiltinField("span_id", span.span_id),
      toTracePagePath(span.trace_id, span.span_id),
    ),
    linkField(
      toUnnamedField("parent_span_id", span.parent_span_id),
      span.parent_span_id && toTracePagePath(span.trace_id, span.parent_span_id),
    ),
  ];
  return [
    { title: "Span", fields: spanFields.filter((field) => field.value !== null) },
    { title: "Attributes", fields: listAttributeFields(span.attributes) },
    { title: "Resource", fields: listResourceFields(span.resource) },
  ];
}

function toStatusName(status: number): string {
  return SPAN_STATUS_NAMES[status] ?? String(status);
}
