// Writes pieces of the query language of `siner-query`: field names, value
// literals, and edits of a query as typed. The rules follow `Display for
// Field` and `quote` in crates/query/src/lib.rs.

import type { Json } from "./api";

/** The built-in fields of every signal. An attribute with one of these names
 * is written as `attr.<key>`. */
const BUILTINS = new Set([
  "service",
  "level",
  "body",
  "trace_id",
  "span_id",
  "source",
  "name",
  "kind",
  "status",
  "error",
  "duration",
  "root",
  "unit",
]);

const KEYWORDS = new Set(["and", "or", "not"]);

const isWordChar = (c: string) => /[\p{L}\p{N}_.\-/:@]/u.test(c);

/** Whether `key` reads back as one word that is not a number. */
function isPlainKey(key: string): boolean {
  const [first, ...rest] = Array.from(key);
  return first !== undefined && /[\p{L}_@]/u.test(first) && rest.every(isWordChar);
}

/** The name a query uses for a record attribute. */
export function attributeField(key: string): string {
  if (!isPlainKey(key)) return `\`${key}\``;
  if (
    key.startsWith("resource.") ||
    key.startsWith("attr.") ||
    KEYWORDS.has(key.toLowerCase()) ||
    BUILTINS.has(key)
  ) {
    return `attr.${key}`;
  }
  return key;
}

/** The name a query uses for a resource attribute, or `undefined` when the
 * language cannot name the key. */
export function resourceField(key: string): string | undefined {
  return Array.from(key).every(isWordChar) && key !== "" ? `resource.${key}` : undefined;
}

/** A string literal, with the escapes the lexer reads. */
export function quote(text: string): string {
  const escaped = text
    .replace(/[\\"]/g, (c) => `\\${c}`)
    .replace(/\n/g, "\\n")
    .replace(/\t/g, "\\t");
  return `"${escaped}"`;
}

/** The literal of an attribute value, or `undefined` for arrays, objects,
 * and null, which a comparison cannot match. */
export function literal(value: Json): string | undefined {
  switch (typeof value) {
    case "string":
      return quote(value);
    case "number":
      return Number.isFinite(value) ? String(value) : undefined;
    case "boolean":
      return String(value);
    default:
      return undefined;
  }
}

/** Whether the query has an `OR` outside of parentheses and strings, so a
 * term joined to it with `AND` needs the query in parentheses. */
function hasTopLevelOr(q: string): boolean {
  let depth = 0;
  let quoteChar: string | undefined;
  let word = "";
  const endWord = () => {
    if (depth === 0 && word.toLowerCase() === "or") return true;
    word = "";
    return false;
  };
  for (let i = 0; i < q.length; i++) {
    const c = q.charAt(i);
    if (quoteChar) {
      if (c === "\\") i++;
      else if (c === quoteChar) quoteChar = undefined;
      continue;
    }
    if (isWordChar(c)) {
      word += c;
      continue;
    }
    if (endWord()) return true;
    if (c === '"' || c === "'" || c === "`") quoteChar = c;
    else if (c === "(") depth++;
    else if (c === ")") depth = Math.max(0, depth - 1);
  }
  return endWord();
}

/** The query with `term` joined to it by `AND`. A query that already ends in
 * the term stays as it is. */
export function addTerm(q: string, term: string): string {
  const current = q.trim();
  if (current === "") return term;
  if (current === term || current.endsWith(` ${term}`)) return current;
  return hasTopLevelOr(current) ? `(${current}) ${term}` : `${current} ${term}`;
}

/** The words of a message template without its placeholders, as a `body ~`
 * term that finds the lines of the group, or `undefined` when no word is
 * left. The words are matched with full-text search, so the term can also
 * find lines of other templates with the same words. */
export function templateTerm(template: string): string | undefined {
  const words = template
    .split(/\s+/)
    .filter((word) => word !== "" && !/<(num|str|uuid|hex)>/.test(word))
    .filter((word) => /[\p{L}\p{N}]/u.test(word));
  return words.length === 0 ? undefined : `body ~ ${quote(words.join(" "))}`;
}
