// What the calls of a service go to, as the calls API names them: a
// database, a host, an RPC service, or a message destination.

import type { Attributes, TargetKey, TargetType } from "@otelo/api";
import { databaseName } from "@otelo/icons";

import { databaseId } from "../semantics";

const TYPE_NAMES: Record<TargetType, string> = {
  database: "Database",
  http: "HTTP",
  rpc: "RPC",
  messaging: "Messaging",
  other: "Other",
};

/** The system of a target by its name, such as `PostgreSQL` or `grpc`, or
 * the name of its type when it has no system, such as `HTTP`. */
export function targetSystem(key: TargetKey): string {
  if (key.system === null) return TYPE_NAMES[key.type];
  return key.type === "database" ? databaseName(databaseId(key.system)) : key.system;
}

/** A target in a few words, such as `PostgreSQL app` or `api.stripe.com`,
 * for a legend or a title. */
export function targetLabel(key: TargetKey): string {
  if (key.type === "http" || key.type === "other") return key.name ?? targetSystem(key);
  return key.name === null ? targetSystem(key) : `${targetSystem(key)} ${key.name}`;
}

export const sameTarget = (a: TargetKey, b: TargetKey) =>
  a.type === b.type && a.system === b.system && a.name === b.name;

/** The parameters of the calls API that name a target, without the ones it
 * lacks. */
export const targetParams = (key: TargetKey) => ({
  type: key.type,
  ...(key.system === null ? {} : { system: key.system }),
  ...(key.name === null ? {} : { target: key.name }),
});

const isType = (type: string): type is TargetType => Object.hasOwn(TYPE_NAMES, type);

/** A target from the parameters of a URL, or `undefined` for a type the API
 * does not know. */
export function parseTarget(
  type: string | undefined,
  system: string | undefined,
  name: string | undefined,
): TargetKey | undefined {
  if (type === undefined || !isType(type)) return undefined;
  return { type, system: system ?? null, name: name ?? null };
}

/**
 * The attributes that show a call as a span that does what its summary
 * says: the summary as the query of a database call, and its path as the
 * route of an HTTP request. A summary takes the values out of a query and
 * the ids out of a path, which the attributes of one call keep.
 */
export function summaryAttributes(
  type: TargetType,
  summary: string,
  attributes: Attributes,
): Attributes {
  if (type === "database") return { ...attributes, "db.query.text": summary };
  const path = summary.slice(summary.indexOf(" ") + 1);
  if (type === "http" && path.startsWith("/")) return { ...attributes, "http.route": path };
  return attributes;
}
