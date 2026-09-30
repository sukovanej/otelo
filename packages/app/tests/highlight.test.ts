import { expect, test } from "vitest";

import type { TokenKind } from "@otelo/ui";

import { highlight } from "../src/highlight";

// The snapshots of crates/query/tests/highlight.rs: after a header, a query
// after "> ", and under it one mark for each of its characters.
const snapshots = import.meta.glob<string>(
  "../../../crates/query/tests/snapshots/highlight__tokens@*.snap",
  { query: "?raw", import: "default", eager: true },
);

const MARKS: Record<TokenKind, string> = {
  field: "f",
  word: "w",
  operator: "o",
  keyword: "k",
  string: "s",
  number: "n",
  boolean: "b",
  punctuation: "p",
  invalid: "x",
};

/** One mark for each character of `query`: of the token it is in, or a space. */
function markEachChar(query: string): string {
  const tokens = highlight(query);
  let marks = "";
  let index = 0;
  for (const c of query) {
    const token = tokens.find((t) => t.start <= index && index < t.end);
    marks += token ? MARKS[token.kind] : " ";
    index += c.length;
  }
  return marks;
}

test("the snapshots of the Rust highlighter are found", () => {
  expect(Object.keys(snapshots).length).toBeGreaterThan(0);
});

test.each(Object.entries(snapshots))(
  "highlight marks the queries of %s as Rust does",
  (_, snap) => {
    const lines = snap.split("\n");
    const queries = lines.flatMap((line, i) =>
      line.startsWith("> ") ? [{ query: line.slice(2), marks: (lines[i + 1] ?? "").slice(2) }] : [],
    );
    expect(queries.length).toBeGreaterThan(0);
    for (const { query, marks } of queries) {
      expect(`${query}\n${markEachChar(query).trimEnd()}`).toBe(`${query}\n${marks}`);
    }
  },
);
