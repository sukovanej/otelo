// The daemon's HTTP API, as `/api/openapi.json` describes it. Only the
// endpoints the UI reads are here.

export type Signal = "logs" | "spans" | "metrics";

export type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

export interface LogLine {
  /** RFC 3339 with nanoseconds. */
  time: string;
  service: string;
  severity: number;
  level: string;
  body: string;
  trace_id: string | null;
  span_id: string | null;
  attributes: Record<string, Json>;
  resource: Record<string, Json>;
  source: string;
}

export interface Logs {
  logs: LogLine[];
  truncated: boolean;
  unindexed: string[];
}

export interface LogGroup {
  template: string;
  count: number;
  severity: number;
  level: string;
  services: string[];
  first: string;
  last: string;
  samples: string[];
}

export interface LogGroups {
  groups: LogGroup[];
  truncated: boolean;
  scanned: number;
  partial: boolean;
  unindexed: string[];
}

export interface SpanEvent {
  /** Nanoseconds since the Unix epoch, to about a microsecond in a number. */
  ts: number;
  name: string;
  attributes: Record<string, Json>;
}

export interface Span {
  trace_id: string;
  span_id: string;
  parent_span_id: string | null;
  service: string;
  name: string;
  /** The OpenTelemetry span kind: 1 internal to 5 consumer, 0 unspecified. */
  kind: number;
  /** RFC 3339 with nanoseconds: the start of the span. */
  time: string;
  duration_ns: number;
  /** The OpenTelemetry status code: 0 unset, 1 ok, 2 error. */
  status: number;
  error: boolean;
  attributes: Record<string, Json>;
  events: SpanEvent[];
  resource: Record<string, Json>;
}

export interface Spans {
  spans: Span[];
  truncated: boolean;
  unindexed: string[];
}

/** A trace by its root span. */
export interface TraceSummary {
  trace_id: string;
  /** The start of the root span. */
  time: string;
  service: string;
  /** The name of the root span. */
  name: string;
  /** The kind of the root span. */
  kind: number;
  duration_ns: number;
  spans: number;
  /** Whether a span of the trace failed. */
  error: boolean;
  /** The attributes of the root span. */
  attributes: Record<string, Json>;
  /** The resource that sent the root span. */
  resource: Record<string, Json>;
}

export interface Traces {
  traces: TraceSummary[];
  truncated: boolean;
  unindexed: string[];
}

/** One trace: its spans by start time, and its logs, newest first. */
export interface Trace {
  trace_id: string;
  spans: Span[];
  logs: LogLine[];
  truncated: boolean;
}

/** The names of span kinds and statuses, as the query language writes them.
 * The kinds count from 1. */
export const SPAN_KINDS = ["unspecified", "internal", "server", "client", "producer", "consumer"];
export const SPAN_STATUSES = ["unset", "ok", "error"];

export interface Suggestion {
  text: string;
  /** Characters, not UTF-16 code units. */
  start: number;
  end: number;
  kind: "field" | "operator" | "value" | "keyword";
  detail: string | null;
}

export interface IndexList {
  indexes: { signal: string; key: string }[];
}

/** The range, the limit, and the query of a list. */
export interface ListParams {
  q: string;
  since: string;
  until: string;
  limit: number;
}

/** An error the daemon sent, with the message of its body. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(method: string, path: string, signal?: AbortSignal): Promise<T> {
  const response = await fetch(path, { method, signal, headers: { accept: "application/json" } });
  if (!response.ok) {
    let message = `${response.status} ${response.statusText}`;
    try {
      // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- the daemon's error body
      const body = (await response.json()) as { error?: string };
      if (body.error) message = body.error;
    } catch {
      // The body was not the JSON of an error; keep the status line.
    }
    throw new ApiError(response.status, message);
  }
  // The daemon answers each path with the type its OpenAPI spec names, which
  // the types here copy.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return (await response.json()) as T;
}

/** A query string of the parameters that are set. */
export function search(params: Record<string, string | number | undefined>): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== "") query.set(key, String(value));
  }
  const text = query.toString();
  return text ? `?${text}` : "";
}

function listSearch(params: ListParams): string {
  return search({
    q: params.q.trim(),
    since: params.since,
    until: params.until,
    limit: params.limit,
  });
}

export const getLogs = (params: ListParams, signal?: AbortSignal) =>
  request<Logs>("GET", `/api/logs${listSearch(params)}`, signal);

export const getLogGroups = (params: ListParams, signal?: AbortSignal) =>
  request<LogGroups>("GET", `/api/logs/groups${listSearch(params)}`, signal);

export const getSpans = (params: ListParams, signal?: AbortSignal) =>
  request<Spans>("GET", `/api/spans${listSearch(params)}`, signal);

export const getTraces = (params: ListParams, signal?: AbortSignal) =>
  request<Traces>("GET", `/api/traces${listSearch(params)}`, signal);

/** One trace from the whole retention. */
export const getTrace = (id: string, signal?: AbortSignal) =>
  request<Trace>("GET", `/api/traces/${encodeURIComponent(id)}`, signal);

export const complete = (kind: Signal, q: string, cursor: number, signal?: AbortSignal) =>
  request<{ suggestions: Suggestion[] }>(
    "GET",
    `/api/complete${search({ signal: kind, q, cursor })}`,
    signal,
  ).then((body) => body.suggestions);

export const addIndex = (kind: Signal, key: string) =>
  request<IndexList>("PUT", `/api/indexes/${kind}/${encodeURIComponent(key)}`);

/** Whether `error` is the rejection of a fetch that was aborted. */
export const aborted = (error: unknown) =>
  error instanceof DOMException && error.name === "AbortError";
