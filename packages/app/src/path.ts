import { toQueryString } from "@otelo/api";

export function toServicePagePath(service: string): string {
  return `/services/${encodeURIComponent(service)}`;
}

export function toTracePagePath(traceId: string, spanId?: string): string {
  return `/traces/${encodeURIComponent(traceId)}${toQueryString({ span: spanId })}`;
}

export function decodePathSegment(segment: string): string {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}
