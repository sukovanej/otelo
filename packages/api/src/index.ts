import type { components, paths } from "./schema";

export type AttributeValue = Schemas["AttributeValue"];
export type Attributes = Schemas["Attributes"];
export type CallOperation = Schemas["CallOperation"];
export type Calls = Schemas["Calls"];
export type FieldBody = Schemas["FieldBody"];
export type FieldSource = Schemas["FieldSource"];
export type LogGroup = Schemas["LogGroup"];
export type LogGroups = Schemas["LogGroups"];
export type LogLine = Schemas["LogLine"];
export type Logs = Schemas["Logs"];
export type Operation = Schemas["Operation"];
export type OperationDetail = Schemas["OperationDetail"];
export type RequestBucket = Schemas["RequestBucket"];
export type Requests = Schemas["Requests"];
export type Service = Schemas["Service"];
export type ServiceSummary = Schemas["ServiceSummary"];
export type Signal = Schemas["Signal"];
export type Spans = Schemas["Spans"];
export type Target = Schemas["Target"];
export type TargetKey = Schemas["TargetKey"];
export type TargetType = Schemas["TargetType"];
export type TraceSpan = Schemas["TraceSpan"];
export type TraceSummary = Schemas["TraceSummary"];
export type Traces = Schemas["Traces"];

export interface ListQuery extends GetQuery<"/api/logs"> {}

type Schemas = components["schemas"];

type ErrorBody = Schemas["ErrorBody"];

type HttpMethod = "get" | "put" | "post" | "delete";

type OkBody<P extends keyof paths, M extends HttpMethod> = paths[P][M] extends {
  responses: { 200: { content: { "application/json": infer T } } };
}
  ? T
  : never;

type GetQuery<P extends keyof paths> = paths[P]["get"] extends {
  parameters: { query?: infer Q };
}
  ? NonNullable<Q>
  : never;

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
  requestJson<OkBody<"/api/logs", "get">>("GET", `/api/logs${search(query)}`, signal);

export const getLogGroups = (query: GetQuery<"/api/logs/groups">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/logs/groups", "get">>("GET", `/api/logs/groups${search(query)}`, signal);

export const getSpans = (query: GetQuery<"/api/spans">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/spans", "get">>("GET", `/api/spans${search(query)}`, signal);

export const getTraces = (query: GetQuery<"/api/traces">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/traces", "get">>("GET", `/api/traces${search(query)}`, signal);

export const getTrace = (
  id: string,
  query: GetQuery<"/api/traces/{trace_id}">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/traces/{trace_id}", "get">>(
    "GET",
    `/api/traces/${encodeURIComponent(id)}${search(query)}`,
    signal,
  );

export const getServices = (query: GetQuery<"/api/services">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/services", "get">>("GET", `/api/services${search(query)}`, signal);

export const getService = (
  name: string,
  query: GetQuery<"/api/services/{name}">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/services/{name}", "get">>(
    "GET",
    `/api/services/${encodeURIComponent(name)}${search(query)}`,
    signal,
  );

export const getOperation = (
  service: string,
  query: GetQuery<"/api/services/{name}/operation">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/services/{name}/operation", "get">>(
    "GET",
    `/api/services/${encodeURIComponent(service)}/operation${search(query)}`,
    signal,
  );

export const getCalls = (
  service: string,
  query: GetQuery<"/api/services/{name}/calls">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/services/{name}/calls", "get">>(
    "GET",
    `/api/services/${encodeURIComponent(service)}/calls${search(query)}`,
    signal,
  );

export const getCall = (
  service: string,
  query: GetQuery<"/api/services/{name}/call">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/services/{name}/call", "get">>(
    "GET",
    `/api/services/${encodeURIComponent(service)}/call${search(query)}`,
    signal,
  );

/** What can go at `cursorInChars` of `query`, and the field of the term the
 * cursor is in. */
export const complete = (
  signal: Signal,
  query: string,
  cursorInChars: number,
  abortSignal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/complete", "get">>(
    "GET",
    `/api/complete${search({ signal, q: query, cursor: cursorInChars })}`,
    abortSignal,
  );

export const addIndex = (signal: Signal, key: string) =>
  requestJson<OkBody<"/api/indexes/{signal}/{key}", "put">>(
    "PUT",
    `/api/indexes/${signal}/${encodeURIComponent(key)}`,
  );

/** Whether `error` is the rejection of a fetch that was aborted. */
export const aborted = (error: unknown) =>
  error instanceof DOMException && error.name === "AbortError";

class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function requestJson<T>(method: string, path: string, signal?: AbortSignal): Promise<T> {
  const response = await fetch(path, {
    method,
    signal: signal ?? null,
    headers: { accept: "application/json" },
  });
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
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- the spec names the type of each path
  return (await response.json()) as T;
}
