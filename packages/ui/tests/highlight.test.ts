import { expect, test } from "vitest";

import { type QueryToken, splitIntoPieces } from "../src/highlight";

test("splitIntoPieces keeps the text between the tokens and after them", () => {
  const field: QueryToken = { start: 1, end: 2, kind: "field" };
  const operator: QueryToken = { start: 3, end: 4, kind: "operator" };
  const number: QueryToken = { start: 6, end: 7, kind: "number" };
  expect(splitIntoPieces(" a =  1 ", [field, operator, number])).toEqual([
    { text: " ", token: undefined },
    { text: "a", token: field },
    { text: " ", token: undefined },
    { text: "=", token: operator },
    { text: "  ", token: undefined },
    { text: "1", token: number },
    { text: " ", token: undefined },
  ]);
});

test("splitIntoPieces of a text without tokens is the text", () => {
  expect(splitIntoPieces("  ", [])).toEqual([{ text: "  ", token: undefined }]);
  expect(splitIntoPieces("", [])).toEqual([]);
});
