// What a span is, read from the OpenTelemetry semantic conventions of its
// attributes and its resource: an HTTP request, a database call, and the
// language of the service. The older names of each attribute count too.

export interface HttpSpan {
  type: "http";
  /** Such as `GET`, upper case. */
  method?: string;
  /** The route, or the path when there is no route. */
  route?: string;
  status?: number;
}

export interface DbSpan {
  type: "db";
  /** Such as `postgresql` or `sqlite`. */
  system: string;
  /** Such as `SELECT`, upper case. */
  operation?: string;
  /** The query, with its whitespace folded. */
  query?: string;
}

export type SpanMeaning = HttpSpan | DbSpan | { type: "other" };

type Attributes = Record<string, unknown>;

/** The first of `keys` that holds a string or a number, as a string. */
function text(attributes: Attributes, ...keys: string[]): string | undefined {
  for (const key of keys) {
    const value = attributes[key];
    if (typeof value === "string" && value !== "") return value;
    if (typeof value === "number") return String(value);
  }
  return undefined;
}

/** The path of a URL, or `undefined` when it does not parse. */
function path(url: string | undefined): string | undefined {
  if (!url) return undefined;
  try {
    return new URL(url).pathname;
  } catch {
    return undefined;
  }
}

/** The SQL keyword a query starts with, such as `SELECT`. */
function keyword(query: string | undefined): string | undefined {
  const word = /^\s*(?:\(\s*)?([a-z]+)/i.exec(query ?? "")?.[1];
  return word?.toUpperCase();
}

/** What a span with these attributes is. */
export function spanMeaning(attributes: Attributes): SpanMeaning {
  const method = text(attributes, "http.request.method", "http.method");
  const status = text(attributes, "http.response.status_code", "http.status_code");
  if (method !== undefined || status !== undefined || "url.full" in attributes) {
    const code = Number(status);
    return {
      type: "http",
      method: method?.toUpperCase(),
      route:
        text(attributes, "http.route", "url.template", "url.path", "http.target") ??
        path(text(attributes, "url.full", "http.url")),
      status: Number.isInteger(code) && code > 0 ? code : undefined,
    };
  }
  const system = text(attributes, "db.system.name", "db.system");
  if (system !== undefined) {
    const query = text(attributes, "db.query.text", "db.statement");
    return {
      type: "db",
      system,
      operation:
        text(attributes, "db.operation.name", "db.operation")?.toUpperCase() ?? keyword(query),
      query: query?.replace(/\s+/g, " ").trim(),
    };
  }
  return { type: "other" };
}

/** The part of a span name that a badge of its meaning does not already
 * say: `SELECT carts` is `carts` after a `SELECT` badge, and `GET /users`
 * is nothing after a badge of `GET` and `/users`. */
export function nameRest(name: string, meaning: SpanMeaning): string {
  if (meaning.type === "other") return name;
  const lead = meaning.type === "http" ? meaning.method : meaning.operation;
  let rest = name.trim();
  if (lead && rest.toUpperCase().startsWith(lead) && /^\s|^$/.test(rest.slice(lead.length))) {
    rest = rest.slice(lead.length).trim();
  }
  if (meaning.type === "http" && rest === meaning.route) return "";
  return rest;
}

/** The ids of `@siner/icons` for the values of `db.system.name` and of the
 * older `db.system` that are not the id itself. */
const DATABASE_IDS: Record<string, string> = {
  "microsoft.sql_server": "mssql",
  "oracle.db": "oracle",
};

/** A database system as an id of the database icons of `@siner/icons`, such
 * as `postgresql` or `mssql`. */
export const databaseId = (system: string) => DATABASE_IDS[system] ?? system;

/** The ids of `@siner/icons` for the values of `telemetry.sdk.language`
 * that name a runtime or a platform, not the language. */
const LANGUAGE_IDS: Record<string, string> = {
  nodejs: "javascript",
  webjs: "javascript",
  dotnet: "csharp",
  elixir: "erlang",
};

/** The language of the service that sent a resource, as an id of the
 * language icons of `@siner/icons`, such as `rust` or `javascript`. */
export function language(resource: Attributes): string | undefined {
  const name = text(resource, "telemetry.sdk.language", "process.runtime.name")?.toLowerCase();
  return name === undefined ? undefined : (LANGUAGE_IDS[name] ?? name);
}
