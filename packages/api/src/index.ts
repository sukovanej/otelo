import type { components, paths } from "./schema";

export type AttributeValue = Schemas["AttributeValue"];
export type Attributes = Schemas["Attributes"];
export type CallOperation = Schemas["CallOperation"];
export type Calls = Schemas["Calls"];
export type FieldBody = Schemas["FieldBody"];
export type FieldSource = FieldBody["source"];
export type IndexedSignal = Schemas["IndexedSignal"];
export type LogGroup = Schemas["LogGroup"];
export type LogGroups = Schemas["LogGroups"];
export type LogLine = Schemas["LogLine"];
export type Logs = Schemas["Logs"];
export type MetricList = Schemas["MetricList"];
export type MetricSeries = Schemas["MetricSeries"];
export type Operation = Schemas["Operation"];
export type OperationDetail = Schemas["OperationDetail"];
export type RequestBucket = Schemas["RequestBucket"];
export type SeriesGroup = Schemas["SeriesGroup"];
export type SeriesInfo = Schemas["SeriesInfo"];
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

export function toQueryString(params: Record<string, string | number | null | undefined>): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") query.set(key, String(value));
  }
  const text = query.toString();
  return text ? `?${text}` : "";
}

export const getLogs = (query: ListQuery, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/logs", "get">>("GET", `/api/logs${toQueryString(query)}`, signal);

export const getLogGroups = (query: GetQuery<"/api/logs/groups">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/logs/groups", "get">>(
    "GET",
    `/api/logs/groups${toQueryString(query)}`,
    signal,
  );

export const getSpans = (query: GetQuery<"/api/spans">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/spans", "get">>("GET", `/api/spans${toQueryString(query)}`, signal);

export const getTraces = (query: GetQuery<"/api/traces">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/traces", "get">>("GET", `/api/traces${toQueryString(query)}`, signal);

export const getTrace = (
  id: string,
  query: GetQuery<"/api/traces/{trace_id}">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/traces/{trace_id}", "get">>(
    "GET",
    `/api/traces/${encodeURIComponent(id)}${toQueryString(query)}`,
    signal,
  );

export const getMetrics = (query: GetQuery<"/api/metrics">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/metrics", "get">>("GET", `/api/metrics${toQueryString(query)}`, signal);

export const getMetricSeries = (
  name: string,
  query: GetQuery<"/api/metrics/{name}">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/metrics/{name}", "get">>(
    "GET",
    `/api/metrics/${encodeURIComponent(name)}${toQueryString(query)}`,
    signal,
  );

export const getServices = (query: GetQuery<"/api/services">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/services", "get">>(
    "GET",
    `/api/services${toQueryString(query)}`,
    signal,
  );

export const getService = (
  name: string,
  query: GetQuery<"/api/services/{name}">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/services/{name}", "get">>(
    "GET",
    `/api/services/${encodeURIComponent(name)}${toQueryString(query)}`,
    signal,
  );

export const getOperation = (
  service: string,
  query: GetQuery<"/api/services/{name}/operation">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/services/{name}/operation", "get">>(
    "GET",
    `/api/services/${encodeURIComponent(service)}/operation${toQueryString(query)}`,
    signal,
  );

export const getCalls = (
  service: string,
  query: GetQuery<"/api/services/{name}/calls">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/services/{name}/calls", "get">>(
    "GET",
    `/api/services/${encodeURIComponent(service)}/calls${toQueryString(query)}`,
    signal,
  );

export const getCall = (
  service: string,
  query: GetQuery<"/api/services/{name}/call">,
  signal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/services/{name}/call", "get">>(
    "GET",
    `/api/services/${encodeURIComponent(service)}/call${toQueryString(query)}`,
    signal,
  );

export const completeQuery = (
  signal: Signal,
  query: string,
  cursorInChars: number,
  abortSignal?: AbortSignal,
) =>
  requestJson<OkBody<"/api/complete", "get">>(
    "GET",
    `/api/complete${toQueryString({ signal, q: query, cursor: cursorInChars })}`,
    abortSignal,
  );

export const addIndex = (signal: IndexedSignal, key: string) =>
  requestJson<OkBody<"/api/indexes/{signal}/{key}", "put">>(
    "PUT",
    `/api/indexes/${signal}/${encodeURIComponent(key)}`,
  );

export const isAbortError = (error: unknown) =>
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
