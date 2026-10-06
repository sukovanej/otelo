import type { components, paths } from "./schema";

export type AttributeKeys = Schemas["AttributeKeys"];
export type AttributeValue = Schemas["AttributeValue"];
export type Attributes = Schemas["Attributes"];
export type ChartKind = Schemas["ChartKind"];
export type Dashboard = Schemas["Dashboard"];
export type DashboardDefinition = Schemas["DashboardDefinition"];
export type DashboardSummary = Schemas["DashboardSummary"];
export type FieldBody = Schemas["FieldBody"];
export type FieldSource = FieldBody["source"];
export type IndexedSignal = Schemas["IndexedSignal"];
export type GroupedQuery = Schemas["GroupedQuery"];
export type LogCounts = Schemas["LogCounts"];
export type LogGroup = Schemas["LogGroup"];
export type LogGroups = Schemas["LogGroups"];
export type LogLine = Schemas["LogLine"];
export type Logs = Schemas["Logs"];
export type MetricAggregation = Schemas["MetricAggregation"];
export type MetricList = Schemas["MetricList"];
export type MetricSeries = Schemas["MetricSeries"];
export type RankOrder = Schemas["RankOrder"];
export type SeriesGroup = Schemas["SeriesGroup"];
export type SeriesInfo = Schemas["SeriesInfo"];
export type Service = Schemas["Service"];
export type ServiceSummary = Schemas["ServiceSummary"];
export type Signal = Schemas["Signal"];
export type SpanBucket = Schemas["SpanBucket"];
export type SpanGroup = Schemas["SpanGroup"];
export type SpanGroupRank = Schemas["SpanGroupRank"];
export type SpanGroups = Schemas["SpanGroups"];
export type SpanMeasure = Schemas["SpanMeasure"];
export type SpanSort = Schemas["SpanSort"];
export type SpanStats = Schemas["SpanStats"];
export type Spans = Schemas["Spans"];
export type TraceSpan = Schemas["TraceSpan"];
export type TraceSummary = Schemas["TraceSummary"];
export type Traces = Schemas["Traces"];
export type Widget = Schemas["Widget"];
export type WidgetDisplay = Schemas["WidgetDisplay"];
export type WidgetLayout = Schemas["WidgetLayout"];
export type WidgetQuery = Schemas["WidgetQuery"];

export interface ListQuery extends GetQuery<"/api/logs"> {}

type Schemas = components["schemas"];

type ErrorBody = Schemas["ErrorBody"];

type LoginBody = Schemas["LoginBody"];

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

export function toQueryString(
  params: Record<string, string | number | boolean | null | undefined>,
): string {
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

export const getLogCounts = (query: GetQuery<"/api/logs/counts">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/logs/counts", "get">>(
    "GET",
    `/api/logs/counts${toQueryString(query)}`,
    signal,
  );

export const getSpans = (query: GetQuery<"/api/spans">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/spans", "get">>("GET", `/api/spans${toQueryString(query)}`, signal);

export const getSpanGroups = (query: GetQuery<"/api/spans/groups">, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/spans/groups", "get">>(
    "GET",
    `/api/spans/groups${toQueryString(query)}`,
    signal,
  );

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

export const getAttributeKeys = (signal: Signal, abortSignal?: AbortSignal) =>
  requestJson<OkBody<"/api/attributes", "get">>(
    "GET",
    `/api/attributes${toQueryString({ signal })}`,
    abortSignal,
  );

export const completeQuery = (query: GetQuery<"/api/complete">, abortSignal?: AbortSignal) =>
  requestJson<OkBody<"/api/complete", "get">>(
    "GET",
    `/api/complete${toQueryString(query)}`,
    abortSignal,
  );

export const listDashboards = (signal?: AbortSignal) =>
  requestJson<OkBody<"/api/dashboards", "get">>("GET", "/api/dashboards", signal);

export const getDashboard = (id: number, signal?: AbortSignal) =>
  requestJson<OkBody<"/api/dashboards/{id}", "get">>("GET", `/api/dashboards/${id}`, signal);

export const createDashboard = (definition: DashboardDefinition) =>
  requestJson<OkBody<"/api/dashboards", "post">>("POST", "/api/dashboards", undefined, definition);

export const replaceDashboard = (id: number, definition: DashboardDefinition) =>
  requestJson<OkBody<"/api/dashboards/{id}", "put">>(
    "PUT",
    `/api/dashboards/${id}`,
    undefined,
    definition,
  );

export const deleteDashboard = (id: number) =>
  requestJson<OkBody<"/api/dashboards/{id}", "delete">>("DELETE", `/api/dashboards/${id}`);

export const addIndex = (signal: IndexedSignal, key: string) =>
  requestJson<OkBody<"/api/indexes/{signal}/{key}", "put">>(
    "PUT",
    `/api/indexes/${signal}/${encodeURIComponent(key)}`,
  );

export const logIn = (password: string) =>
  sendRequest("POST", "/api/login", undefined, { password } satisfies LoginBody).then(
    () => undefined,
  );

export const logOut = () => sendRequest("POST", "/api/logout").then(() => undefined);

export const isAbortError = (error: unknown) =>
  error instanceof DOMException && error.name === "AbortError";

export const isUnauthorizedError = (error: unknown) =>
  error instanceof ApiError && error.status === 401;

class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

async function requestJson<T>(
  method: string,
  path: string,
  signal?: AbortSignal,
  requestBody?: object,
): Promise<T> {
  const response = await sendRequest(method, path, signal, requestBody);
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- the spec names the type of each path
  return (await response.json()) as T;
}

async function sendRequest(
  method: string,
  path: string,
  signal?: AbortSignal,
  requestBody?: object,
): Promise<Response> {
  const response = await fetch(path, {
    method,
    signal: signal ?? null,
    headers:
      requestBody === undefined
        ? { accept: "application/json" }
        : { accept: "application/json", "content-type": "application/json" },
    body: requestBody === undefined ? null : JSON.stringify(requestBody),
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
  return response;
}
