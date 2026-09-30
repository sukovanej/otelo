import type { Attributes } from "@otelo/api";

const DATABASE_QUERY_KEYS = ["db.query.text", "db.statement"];

// A `:name` parameter starts a segment; a colon inside one, such as in
// `10:30` or `users:batch`, is text.
const ROUTE_PART_PATTERN = /(\/)|(\{[^}/]*\}|(?<=\/):[A-Za-z_]\w*|<[^>/]*>|\*)/g;

const SQL_SYSTEM_NAMES =
  `actian.ingres aws.redshift clickhouse cockroachdb derby firebirdsql gcp.spanner h2database
  hive hsqldb ibm.db2 ibm.informix ibm.netezza instantdb intersystems.cache mariadb
  microsoft.sql_server mysql oracle.db other_sql postgresql sap.hana sap.maxdb sqlite teradata
  trino`.split(/\s+/);

const OLD_SQL_SYSTEM_NAMES =
  `cache cloudscape db2 edb firebird firstsql h2 hanadb informix ingres interbase
  intersystems_cache maxdb mssql mssqlcompact netezza oracle pervasive pointbase progress redshift
  spanner sybase vertica`.split(/\s+/);

const SQL_SYSTEMS = new Set([...SQL_SYSTEM_NAMES, ...OLD_SQL_SYSTEM_NAMES]);

const DATABASE_ICON_IDS: Record<string, string> = {
  "microsoft.sql_server": "mssql",
  "oracle.db": "oracle",
};

const LANGUAGE_ICON_IDS: Record<string, string> = {
  nodejs: "javascript",
  webjs: "javascript",
  dotnet: "csharp",
  elixir: "erlang",
};

export type SpanMeaning = HttpSpan | DatabaseSpan | OtherSpan;

export interface HttpSpan {
  readonly kind: "http";
  readonly method: string | undefined;
  readonly route: string | undefined;
  readonly status: number | undefined;
}

export interface DatabaseSpan {
  readonly kind: "database";
  readonly system: string;
  readonly query: string | undefined;
}

export type RoutePartKind = "slash" | "parameter" | "text";

interface OtherSpan {
  readonly kind: "other";
}

interface RoutePart {
  readonly text: string;
  readonly kind: RoutePartKind;
}

export function readSpanMeaning(attributes: Attributes): SpanMeaning {
  const method = readFirstText(attributes, "http.request.method", "http.method");
  const status = readFirstText(attributes, "http.response.status_code", "http.status_code");
  if (method !== undefined || status !== undefined || "url.full" in attributes) {
    const code = Number(status);
    return {
      kind: "http",
      method: method?.toUpperCase(),
      route:
        readFirstText(attributes, "http.route", "url.template", "url.path", "http.target") ??
        parseUrlPath(readFirstText(attributes, "url.full", "http.url")),
      status: Number.isInteger(code) && code > 0 ? code : undefined,
    };
  }
  const system = readFirstText(attributes, "db.system.name", "db.system");
  if (system !== undefined) {
    return { kind: "database", system, query: readFirstText(attributes, ...DATABASE_QUERY_KEYS) };
  }
  return { kind: "other" };
}

export function splitRouteIntoParts(route: string): RoutePart[] {
  const parts: RoutePart[] = [];
  let textStart = 0;
  for (const match of route.matchAll(ROUTE_PART_PATTERN)) {
    if (match.index > textStart) {
      parts.push({ text: route.slice(textStart, match.index), kind: "text" });
    }
    parts.push({ text: match[0], kind: match[1] ? "slash" : "parameter" });
    textStart = match.index + match[0].length;
  }
  if (textStart < route.length) parts.push({ text: route.slice(textStart), kind: "text" });
  return parts;
}

export function isSqlSystem(system: string): boolean {
  return SQL_SYSTEMS.has(system);
}

export function isSqlQueryAttribute(attributes: Attributes, key: string): boolean {
  const system = readFirstText(attributes, "db.system.name", "db.system");
  return system !== undefined && isSqlSystem(system) && DATABASE_QUERY_KEYS.includes(key);
}

export function stripBadgesFromName(name: string, meaning: SpanMeaning): string {
  if (meaning.kind !== "http") return name;
  const method = meaning.method;
  let remaining = name.trim();
  if (
    method &&
    remaining.toUpperCase().startsWith(method) &&
    /^\s|^$/.test(remaining.slice(method.length))
  ) {
    remaining = remaining.slice(method.length).trim();
  }
  if (remaining === meaning.route) return "";
  return remaining;
}

export function toDatabaseIconId(system: string): string {
  return DATABASE_ICON_IDS[system] ?? system;
}

export function readLanguageIconId(resource: Attributes): string | undefined {
  const name = readFirstText(
    resource,
    "telemetry.sdk.language",
    "process.runtime.name",
  )?.toLowerCase();
  return name === undefined ? undefined : (LANGUAGE_ICON_IDS[name] ?? name);
}

function readFirstText(attributes: Attributes, ...keys: string[]): string | undefined {
  for (const key of keys) {
    const value = attributes[key];
    if (typeof value === "string" && value !== "") return value;
    if (typeof value === "number") return String(value);
  }
  return undefined;
}

function parseUrlPath(url: string | undefined): string | undefined {
  if (!url) return undefined;
  try {
    return new URL(url).pathname;
  } catch {
    return undefined;
  }
}
