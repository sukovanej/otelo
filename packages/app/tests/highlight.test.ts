import { expect, test } from "vitest";

import type { TokenKind } from "@otelo/ui";

import { highlightQuery } from "../src/highlight";

// The snapshots of crates/query/tests/highlight.rs: after a header, a query
// after "> ", and under it one mark for each of its characters.
const snapshots = import.meta.glob<string>(
  "../../../crates/query/tests/snapshots/highlight__tokens@*.snap",
  { query: "?raw", import: "default", eager: true },
);

const KIND_MARKS: Record<TokenKind, string> = {
  field: "f",
  undecided: "w",
  operator: "o",
  keyword: "k",
  string: "s",
  number: "n",
  boolean: "b",
  punctuation: "p",
  invalid: "x",
};

interface MarkedQuery {
  readonly query: string;
  readonly marks: string;
}

test("the snapshots of the Rust highlighter are found", () => {
  expect(Object.keys(snapshots).length).toBeGreaterThan(0);
});

test.each(Object.entries(snapshots))(
  "highlightQuery marks the queries of %s as Rust does",
  (_, snapshot) => {
    const markedQueries = readMarkedQueries(snapshot);
    expect(markedQueries.length).toBeGreaterThan(0);
    for (const { query, marks } of markedQueries) {
      expect(`${query}\n${markEachChar(query).trimEnd()}`).toBe(`${query}\n${marks}`);
    }
  },
);

function readMarkedQueries(snapshot: string): MarkedQuery[] {
  const lines = snapshot.split("\n");
  return lines.flatMap((line, index) =>
    line.startsWith("> ")
      ? [{ query: line.slice(2), marks: (lines[index + 1] ?? "").slice(2) }]
      : [],
  );
}

function markEachChar(query: string): string {
  const tokens = highlightQuery(query);
  let marks = "";
  let index = 0;
  for (const char of query) {
    const token = tokens.find(({ start, end }) => start <= index && index < end);
    marks += token ? KIND_MARKS[token.kind] : " ";
    index += char.length;
  }
  return marks;
}
