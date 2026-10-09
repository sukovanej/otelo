import { subscribeToIndexing } from "./events";
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
export type Indexing = Schemas["Indexing"];
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
export type SignalIndexing = Schemas["SignalIndexing"];
export type SpanBucket = Schemas["SpanBucket"];
export type SpanGroup = Schemas["SpanGroup"];
export type SpanGroupRank = Schemas["SpanGroupRank"];
export type SpanGroups = Schemas["SpanGroups"];
export type SpanMeasure = Schemas["SpanMeasure"];
export type SpanSort = Schemas["SpanSort"];
export type SpanStats = Schemas["SpanStats"];
export type Spans = Schemas["Spans"];
export type Trace = Schemas["Trace"];
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

export interface Api {
  readonly getLogs: (query: ListQuery, signal?: AbortSignal) => Promise<OkBody<"/api/logs", "get">>;
  readonly getLogGroups: (
    query: GetQuery<"/api/logs/groups">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/logs/groups", "get">>;
  readonly getLogCounts: (
    query: GetQuery<"/api/logs/counts">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/logs/counts", "get">>;
  readonly getSpans: (
    query: GetQuery<"/api/spans">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/spans", "get">>;
  readonly getSpanGroups: (
    query: GetQuery<"/api/spans/groups">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/spans/groups", "get">>;
  readonly getTraces: (
    query: GetQuery<"/api/traces">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/traces", "get">>;
  readonly getTrace: (
    id: string,
    query: GetQuery<"/api/traces/{trace_id}">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/traces/{trace_id}", "get">>;
  readonly getMetrics: (
    query: GetQuery<"/api/metrics">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/metrics", "get">>;
  readonly getMetricSeries: (
    name: string,
    query: GetQuery<"/api/metrics/{name}">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/metrics/{name}", "get">>;
  readonly getServices: (
    query: GetQuery<"/api/services">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/services", "get">>;
  readonly getService: (
    name: string,
    query: GetQuery<"/api/services/{name}">,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/services/{name}", "get">>;
  readonly getAttributeKeys: (
    signal: Signal,
    abortSignal?: AbortSignal,
  ) => Promise<OkBody<"/api/attributes", "get">>;
  readonly completeQuery: (
    query: GetQuery<"/api/complete">,
    abortSignal?: AbortSignal,
  ) => Promise<OkBody<"/api/complete", "get">>;
  readonly listDashboards: (signal?: AbortSignal) => Promise<OkBody<"/api/dashboards", "get">>;
  readonly getDashboard: (
    id: number,
    signal?: AbortSignal,
  ) => Promise<OkBody<"/api/dashboards/{id}", "get">>;
  readonly createDashboard: (
    definition: DashboardDefinition,
  ) => Promise<OkBody<"/api/dashboards", "post">>;
  readonly replaceDashboard: (
    id: number,
    definition: DashboardDefinition,
  ) => Promise<OkBody<"/api/dashboards/{id}", "put">>;
  readonly deleteDashboard: (id: number) => Promise<OkBody<"/api/dashboards/{id}", "delete">>;
  readonly addIndex: (
    signal: IndexedSignal,
    key: string,
  ) => Promise<OkBody<"/api/indexes/{signal}/{key}", "put">>;
  readonly logIn: (password: string) => Promise<void>;
  readonly logOut: () => Promise<void>;
  readonly subscribeToIndexing: (onIndexing: (indexing: Indexing) => void) => () => void;
}

export function createApi(fetchFromDaemon: typeof fetch): Api {
  const requestJson = async <T>(
    method: string,
    path: string,
    signal?: AbortSignal,
    requestBody?: object,
  ): Promise<T> => {
    const response = await sendRequest(fetchFromDaemon, method, path, signal, requestBody);
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- the spec names the type of each path
    return (await response.json()) as T;
  };
  return {
    getLogs: (query, signal) => requestJson("GET", `/api/logs${toQueryString(query)}`, signal),
    getLogGroups: (query, signal) =>
      requestJson("GET", `/api/logs/groups${toQueryString(query)}`, signal),
    getLogCounts: (query, signal) =>
      requestJson("GET", `/api/logs/counts${toQueryString(query)}`, signal),
    getSpans: (query, signal) => requestJson("GET", `/api/spans${toQueryString(query)}`, signal),
    getSpanGroups: (query, signal) =>
      requestJson("GET", `/api/spans/groups${toQueryString(query)}`, signal),
    getTraces: (query, signal) => requestJson("GET", `/api/traces${toQueryString(query)}`, signal),
    getTrace: (id, query, signal) =>
      requestJson("GET", `/api/traces/${encodeURIComponent(id)}${toQueryString(query)}`, signal),
    getMetrics: (query, signal) =>
      requestJson("GET", `/api/metrics${toQueryString(query)}`, signal),
    getMetricSeries: (name, query, signal) =>
      requestJson("GET", `/api/metrics/${encodeURIComponent(name)}${toQueryString(query)}`, signal),
    getServices: (query, signal) =>
      requestJson("GET", `/api/services${toQueryString(query)}`, signal),
    getService: (name, query, signal) =>
      requestJson(
        "GET",
        `/api/services/${encodeURIComponent(name)}${toQueryString(query)}`,
        signal,
      ),
    getAttributeKeys: (signal, abortSignal) =>
      requestJson("GET", `/api/attributes${toQueryString({ signal })}`, abortSignal),
    completeQuery: (query, abortSignal) =>
      requestJson("GET", `/api/complete${toQueryString(query)}`, abortSignal),
    listDashboards: (signal) => requestJson("GET", "/api/dashboards", signal),
    getDashboard: (id, signal) => requestJson("GET", `/api/dashboards/${id}`, signal),
    createDashboard: (definition) => requestJson("POST", "/api/dashboards", undefined, definition),
    replaceDashboard: (id, definition) =>
      requestJson("PUT", `/api/dashboards/${id}`, undefined, definition),
    deleteDashboard: (id) => requestJson("DELETE", `/api/dashboards/${id}`),
    addIndex: (signal, key) =>
      requestJson("PUT", `/api/indexes/${signal}/${encodeURIComponent(key)}`),
    logIn: (password) =>
      sendRequest(fetchFromDaemon, "POST", "/api/login", undefined, {
        password,
      } satisfies LoginBody).then(() => undefined),
    logOut: () => sendRequest(fetchFromDaemon, "POST", "/api/logout").then(() => undefined),
    subscribeToIndexing,
  };
}

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

async function sendRequest(
  fetchFromDaemon: typeof fetch,
  method: string,
  path: string,
  signal?: AbortSignal,
  requestBody?: object,
): Promise<Response> {
  const response = await fetchFromDaemon(path, {
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
