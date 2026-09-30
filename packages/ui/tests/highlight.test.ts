import { expect, test } from "vitest";

import { pieces, type QueryToken } from "../src/highlight";

test("pieces keeps the text between the tokens and after them", () => {
  const field: QueryToken = { start: 1, end: 2, kind: "field" };
  const operator: QueryToken = { start: 3, end: 4, kind: "operator" };
  const number: QueryToken = { start: 6, end: 7, kind: "number" };
  expect(pieces(" a =  1 ", [field, operator, number])).toEqual([
    { text: " ", token: undefined },
    { text: "a", token: field },
    { text: " ", token: undefined },
    { text: "=", token: operator },
    { text: "  ", token: undefined },
    { text: "1", token: number },
    { text: " ", token: undefined },
  ]);
});

test("pieces of a text without tokens is the text", () => {
  expect(pieces("  ", [])).toEqual([{ text: "  ", token: undefined }]);
  expect(pieces("", [])).toEqual([]);
});
