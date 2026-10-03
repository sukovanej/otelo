import type { AttributeValue } from "@otelo/api";

import { isKeyword, isWordChar, lexQuery } from "./lexer";

const BUILTIN_FIELD_NAMES = new Set([
  "service",
  "level",
  "body",
  "trace_id",
  "span_id",
  "name",
  "kind",
  "status",
  "error",
  "duration",
  "root",
  "unit",
]);

// Follows `Display for Field` and `quote` in crates/query/src/lib.rs.
export function writeAttributeField(key: string): string {
  if (!isBareWord(key)) return `\`${key}\``;
  if (
    key.startsWith("resource.") ||
    key.startsWith("attr.") ||
    isKeyword(key) ||
    BUILTIN_FIELD_NAMES.has(key)
  ) {
    return `attr.${key}`;
  }
  return key;
}

export function writeResourceField(key: string): string | undefined {
  return Array.from(key).every(isWordChar) && key !== "" ? `resource.${key}` : undefined;
}

export function quoteString(text: string): string {
  const escaped = text
    .replace(/[\\"]/g, (char) => `\\${char}`)
    .replace(/\n/g, "\\n")
    .replace(/\t/g, "\\t");
  return `"${escaped}"`;
}

export function writeLiteral(value: AttributeValue): string | undefined {
  if (typeof value === "string") return quoteString(value);
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : undefined;
  if (typeof value === "boolean") return String(value);
  return undefined;
}

export function addTerm(query: string, term: string): string {
  const current = query.trim();
  if (current === "") return term;
  if (current === term || current.endsWith(` ${term}`)) return current;
  return hasTopLevelOr(current) ? `(${current}) ${term}` : `${current} ${term}`;
}

// The words match with full-text search, so the term can also find lines of
// other templates with the same words.
export function writeTemplateTerm(template: string): string | undefined {
  const words = template
    .split(/\s+/)
    .filter((word) => word !== "" && !/<(num|str|uuid|hex)>/.test(word))
    .filter((word) => /[\p{L}\p{N}]/u.test(word));
  return words.length === 0 ? undefined : `body ~ ${quoteString(words.join(" "))}`;
}

function isBareWord(key: string): boolean {
  const [first, ...rest] = Array.from(key);
  return first !== undefined && /[\p{Alphabetic}_@]/u.test(first) && rest.every(isWordChar);
}

function hasTopLevelOr(query: string): boolean {
  let depth = 0;
  for (const { type, start, end } of lexQuery(query)) {
    if (type === "(") depth++;
    else if (type === ")") depth = Math.max(0, depth - 1);
    else if (type === "word" && depth === 0 && query.slice(start, end).toLowerCase() === "or") {
      return true;
    }
  }
  return false;
}
