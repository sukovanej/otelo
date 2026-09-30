// What a span is, read from the OpenTelemetry semantic conventions of its
// attributes and its resource: an HTTP request, a database call, and the
// language of the service. The older names of each attribute count too.

import type { Attributes } from "@otelo/api";

export interface HttpSpan {
  type: "http";
  /** Such as `GET`, upper case. */
  method: string | undefined;
  /** The route, or the path when there is no route. */
  route: string | undefined;
  status: number | undefined;
}

export interface DbSpan {
  type: "db";
  /** Such as `postgresql` or `sqlite`. */
  system: string;
  /** The query as the span has it. */
  query: string | undefined;
}

export type SpanMeaning = HttpSpan | DbSpan | { type: "other" };

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

/** The keys of the query of a database call, the older one last. */
const QUERY_KEYS = ["db.query.text", "db.statement"];

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
    return { type: "db", system, query: text(attributes, ...QUERY_KEYS) };
  }
  return { type: "other" };
}

/** A piece of a route: a slash, a parameter such as `{id}`, `:id`, `<id>`,
 * or `*`, or the fixed text between them. */
export interface RoutePart {
  text: string;
  kind: "slash" | "param" | "text";
}

// A `:name` parameter starts a segment; a colon inside one, such as in
// `10:30` or `users:batch`, is text.
const ROUTE_TOKEN = /(\/)|(\{[^}/]*\}|(?<=\/):[A-Za-z_]\w*|<[^>/]*>|\*)/g;

/** The pieces of a route or a path, such as `/users/{id}`, in order. */
export function routeParts(route: string): RoutePart[] {
  const parts: RoutePart[] = [];
  let at = 0;
  for (const match of route.matchAll(ROUTE_TOKEN)) {
    if (match.index > at) parts.push({ text: route.slice(at, match.index), kind: "text" });
    parts.push({ text: match[0], kind: match[1] ? "slash" : "param" });
    at = match.index + match[0].length;
  }
  if (at < route.length) parts.push({ text: route.slice(at), kind: "text" });
  return parts;
}

/** The database systems of the semantic conventions that speak SQL, by
 * their values of `db.system.name` and then by the ones of the older
 * `db.system` that differ. */
const SQL_SYSTEMS = new Set(
  `actian.ingres aws.redshift clickhouse cockroachdb derby firebirdsql gcp.spanner h2database
  hive hsqldb ibm.db2 ibm.informix ibm.netezza instantdb intersystems.cache mariadb
  microsoft.sql_server mysql oracle.db other_sql postgresql sap.hana sap.maxdb sqlite teradata
  trino
  cache cloudscape db2 edb firebird firstsql h2 hanadb informix ingres interbase
  intersystems_cache maxdb mssql mssqlcompact netezza oracle pervasive pointbase progress redshift
  spanner sybase vertica`.split(/\s+/),
);

/** Whether the queries of a database system are SQL. */
export const isSql = (system: string) => SQL_SYSTEMS.has(system);

/** Whether the attribute `key` of a record with these attributes holds a
 * SQL query: the query of a call to a system that speaks SQL. */
export function isSqlQuery(attributes: Attributes, key: string): boolean {
  const system = text(attributes, "db.system.name", "db.system");
  return system !== undefined && isSql(system) && QUERY_KEYS.includes(key);
}

/** The part of the name of an HTTP span that its badges do not already
 * say: `GET /users` is nothing after a badge of `GET` and `/users`. Any
 * other span keeps its name. */
export function nameRest(name: string, meaning: SpanMeaning): string {
  if (meaning.type !== "http") return name;
  const lead = meaning.method;
  let rest = name.trim();
  if (lead && rest.toUpperCase().startsWith(lead) && /^\s|^$/.test(rest.slice(lead.length))) {
    rest = rest.slice(lead.length).trim();
  }
  if (rest === meaning.route) return "";
  return rest;
}

/** The ids of `@otelo/icons` for the values of `db.system.name` and of the
 * older `db.system` that are not the id itself. */
const DATABASE_IDS: Record<string, string> = {
  "microsoft.sql_server": "mssql",
  "oracle.db": "oracle",
};

/** A database system as an id of the database icons of `@otelo/icons`, such
 * as `postgresql` or `mssql`. */
export const databaseId = (system: string) => DATABASE_IDS[system] ?? system;

/** The ids of `@otelo/icons` for the values of `telemetry.sdk.language`
 * that name a runtime or a platform, not the language. */
const LANGUAGE_IDS: Record<string, string> = {
  nodejs: "javascript",
  webjs: "javascript",
  dotnet: "csharp",
  elixir: "erlang",
};

/** The language of the service that sent a resource, as an id of the
 * language icons of `@otelo/icons`, such as `rust` or `javascript`. */
export function language(resource: Attributes): string | undefined {
  const name = text(resource, "telemetry.sdk.language", "process.runtime.name")?.toLowerCase();
  return name === undefined ? undefined : (LANGUAGE_IDS[name] ?? name);
}
