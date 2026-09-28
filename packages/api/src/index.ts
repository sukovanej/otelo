// The HTTP API of the siner daemon: the types of its spec, which the Rust
// types of the daemon make, by their names there, and a client of the
// endpoints the UI reads.

import type { components, paths } from "./schema";

type Schemas = components["schemas"];

export type Attribute = Schemas["Attribute"];
export type Attributes = Schemas["Attributes"];
export type Bucket = Schemas["Bucket"];
export type CompletionKind = Schemas["CompletionKind"];
export type Completions = Schemas["Completions"];
export type Distribution = Schemas["Distribution"];
export type ErrorBody = Schemas["ErrorBody"];
export type IndexBody = Schemas["IndexBody"];
export type IndexList = Schemas["IndexList"];
export type LogGroup = Schemas["LogGroup"];
export type LogGroups = Schemas["LogGroups"];
export type LogLine = Schemas["LogLine"];
export type Logs = Schemas["Logs"];
export type MetricList = Schemas["MetricList"];
export type MetricSeries = Schemas["MetricSeries"];
export type Series = Schemas["Series"];
export type SeriesInfo = Schemas["SeriesInfo"];
export type Signal = Schemas["Signal"];
export type SpanEvent = Schemas["SpanEvent"];
export type Spans = Schemas["Spans"];
export type SqlRequest = Schemas["SqlRequest"];
export type SqlResult = Schemas["SqlResult"];
export type SuggestionBody = Schemas["SuggestionBody"];
export type Trace = Schemas["Trace"];
export type TraceSpan = Schemas["TraceSpan"];
export type TraceSummary = Schemas["TraceSummary"];
export type Traces = Schemas["Traces"];

type Method = "get" | "put" | "post" | "delete";

/** The JSON that `method` on `path` answers with when it succeeds. */
type Ok<P extends keyof paths, M extends Method> = paths[P][M] extends {
  responses: { 200: { content: { "application/json": infer T } } };
}
  ? T
  : never;

/** The query parameters of a GET on `path`. */
type QueryOf<P extends keyof paths> = paths[P]["get"] extends {
  parameters: { query?: infer Q };
}
  ? NonNullable<Q>
  : never;

/** The range, the limit, and the query of a list: of logs, spans, or
 * traces. */
export type ListQuery = QueryOf<"/api/logs">;

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
      const body = (await response.json()) as Partial<ErrorBody>;
      if (body.error) message = body.error;
    } catch {
      // The body was not the JSON of an error; keep the status line.
    }
    throw new ApiError(response.status, message);
  }
  // The daemon answers each path with the type its spec names, which the
  // callers here take from the generated schema.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return (await response.json()) as T;
}

/** A query string of the parameters that are set, or an empty string. */
export function search(params: Record<string, string | number | null | undefined>): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") query.set(key, String(value));
  }
  const text = query.toString();
  return text ? `?${text}` : "";
}

export const getLogs = (query: ListQuery, signal?: AbortSignal) =>
  request<Ok<"/api/logs", "get">>("GET", `/api/logs${search(query)}`, signal);

export const getLogGroups = (query: QueryOf<"/api/logs/groups">, signal?: AbortSignal) =>
  request<Ok<"/api/logs/groups", "get">>("GET", `/api/logs/groups${search(query)}`, signal);

export const getSpans = (query: QueryOf<"/api/spans">, signal?: AbortSignal) =>
  request<Ok<"/api/spans", "get">>("GET", `/api/spans${search(query)}`, signal);

export const getTraces = (query: QueryOf<"/api/traces">, signal?: AbortSignal) =>
  request<Ok<"/api/traces", "get">>("GET", `/api/traces${search(query)}`, signal);

/** One trace, from the whole retention unless `query` sets a range. */
export const getTrace = (
  id: string,
  query: QueryOf<"/api/traces/{trace_id}"> = {},
  signal?: AbortSignal,
) =>
  request<Ok<"/api/traces/{trace_id}", "get">>(
    "GET",
    `/api/traces/${encodeURIComponent(id)}${search(query)}`,
    signal,
  );

/** What can go at `cursor`, in characters, of the query `q`. */
export const complete = (kind: Signal, q: string, cursor: number, signal?: AbortSignal) =>
  request<Ok<"/api/complete", "get">>(
    "GET",
    `/api/complete${search({ signal: kind, q, cursor })}`,
    signal,
  ).then((body) => body.suggestions);

export const addIndex = (kind: Signal, key: string) =>
  request<Ok<"/api/indexes/{signal}/{key}", "put">>(
    "PUT",
    `/api/indexes/${kind}/${encodeURIComponent(key)}`,
  );

/** Whether `error` is the rejection of a fetch that was aborted. */
export const aborted = (error: unknown) =>
  error instanceof DOMException && error.name === "AbortError";
