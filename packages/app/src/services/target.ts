import type { Attributes, TargetKey, TargetType } from "@otelo/api";
import { databaseName } from "@otelo/icons";

import { toDatabaseIconId } from "../semantics";

const TARGET_TYPE_NAMES: Record<TargetType, string> = {
  database: "Database",
  http: "HTTP",
  rpc: "RPC",
  messaging: "Messaging",
  other: "Other",
};

export function nameTargetSystem(target: TargetKey): string {
  if (target.system === null) return TARGET_TYPE_NAMES[target.type];
  return target.type === "database" ? databaseName(toDatabaseIconId(target.system)) : target.system;
}

export function toTargetLabel(target: TargetKey): string {
  if (target.type === "http" || target.type === "other") {
    return target.name ?? nameTargetSystem(target);
  }
  return target.name === null
    ? nameTargetSystem(target)
    : `${nameTargetSystem(target)} ${target.name}`;
}

export const isSameTarget = (target: TargetKey, other: TargetKey) =>
  target.type === other.type && target.system === other.system && target.name === other.name;

export const toTargetParams = (target: TargetKey) => ({
  type: target.type,
  ...(target.system === null ? {} : { system: target.system }),
  ...(target.name === null ? {} : { target: target.name }),
});

export function parseTarget(
  type: string | undefined,
  system: string | undefined,
  name: string | undefined,
): TargetKey | undefined {
  if (type === undefined || !isTargetType(type)) return undefined;
  return { type, system: system ?? null, name: name ?? null };
}

// A summary takes the values out of a query and the ids out of a path, which
// the attributes of one call keep.
export function applySummaryToAttributes(
  type: TargetType,
  summary: string,
  attributes: Attributes,
): Attributes {
  if (type === "database") return { ...attributes, "db.query.text": summary };
  const path = summary.slice(summary.indexOf(" ") + 1);
  if (type === "http" && path.startsWith("/")) return { ...attributes, "http.route": path };
  return attributes;
}

function isTargetType(type: string): type is TargetType {
  return Object.hasOwn(TARGET_TYPE_NAMES, type);
}
