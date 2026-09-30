// The words of the card that describes a field of a query, from what
// `/api/complete` says of the field.

import type { FieldBody, FieldSource, Signal } from "@otelo/api";

export const SOURCES: Record<FieldSource, string> = {
  builtin: "built-in",
  attribute: "attribute",
  resource: "resource attribute",
};

/** One record of each signal, and several. */
const RECORDS: Record<Signal, [string, string]> = {
  logs: ["log line", "log lines"],
  spans: ["span", "spans"],
  metrics: ["series", "series"],
};

const counted = (n: number, [one, several]: [string, string]) =>
  `${n.toLocaleString()} ${n === 1 ? one : several}`;

/** The type of the field, and how many records or resources have it. */
export function typeLine(field: FieldBody, signal: Signal): string {
  if (field.count === null) return field.type;
  const nouns: [string, string] =
    field.source === "resource" ? ["resource", "resources"] : RECORDS[signal];
  return `${field.type} · ${counted(field.count, nouns)}`;
}

/** What the listed values are: all of them, or the most common of more. */
export function valuesTitle(field: FieldBody): string {
  const all = field.values.length === field.distinct_values && !field.many_values;
  if (all) return counted(field.distinct_values, ["value", "values"]);
  const known = field.distinct_values.toLocaleString();
  return `Most common of ${known}${field.many_values ? "+" : ""} values`;
}
