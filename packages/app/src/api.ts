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
