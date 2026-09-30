import { completeQuery, type FieldBody, type FieldSource, type Signal } from "@otelo/api";
import type { QueryToken } from "@otelo/ui";

export const SOURCE_LABELS: Record<FieldSource, string> = {
  builtin: "built-in",
  attribute: "attribute",
  resource: "resource attribute",
};

const RECORD_NOUNS: Record<Signal, Noun> = {
  logs: { one: "log line", several: "log lines" },
  spans: { one: "span", several: "spans" },
  metrics: { one: "series", several: "series" },
};

const RESOURCE_NOUN: Noun = { one: "resource", several: "resources" };

const VALUE_NOUN: Noun = { one: "value", several: "values" };

interface Noun {
  readonly one: string;
  readonly several: string;
}

export async function describeFieldOfToken(
  signal: Signal,
  query: string,
  token: QueryToken,
  abort: AbortSignal,
): Promise<FieldBody | undefined> {
  if (token.kind !== "field" && token.kind !== "undecided") return undefined;
  // The field alone, so the rest of the query cannot hide it.
  const name = query.slice(token.start, token.end);
  const endInChars = Array.from(name).length;
  const { field } = await completeQuery(signal, name, endInChars, abort);
  return field ?? undefined;
}

export function formatTypeLine(field: FieldBody, signal: Signal): string {
  if (field.source === "builtin") return field.type;
  const noun = field.source === "resource" ? RESOURCE_NOUN : RECORD_NOUNS[signal];
  return `${field.type} · ${formatCount(field.count, noun)}`;
}

export function formatValuesTitle(field: FieldBody): string {
  const allListed = field.values.length === field.distinct_values && !field.has_more_values;
  if (allListed) return formatCount(field.distinct_values, VALUE_NOUN);
  const known = field.distinct_values.toLocaleString();
  return `Most common of ${known}${field.has_more_values ? "+" : ""} values`;
}

function formatCount(count: number, noun: Noun): string {
  return `${count.toLocaleString()} ${count === 1 ? noun.one : noun.several}`;
}
