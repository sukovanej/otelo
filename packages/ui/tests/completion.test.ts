import { describe, expect, test } from "vitest";

import { applySuggestion, toChars, toUtf16 } from "../src/completion";

test("toChars and toUtf16 count a character outside the BMP once", () => {
  const text = "a😀b";
  expect(toChars(text, 3)).toBe(2);
  expect(toUtf16(text, 2)).toBe(3);
  expect(toUtf16(text, 9)).toBe(text.length);
});

describe("applySuggestion", () => {
  test("replaces the word at the cursor and adds a space", () => {
    const next = applySuggestion("http.ro", {
      text: "http.route",
      start: 0,
      end: 7,
      kind: "field",
      detail: null,
    });
    expect(next).toEqual({ text: "http.route ", cursor: 11 });
  });

  test("keeps the space or the parenthesis that follows", () => {
    const next = applySuggestion('x = "/l" y = 1', {
      text: '"/languages"',
      start: 4,
      end: 8,
      kind: "value",
      detail: "40",
    });
    expect(next).toEqual({ text: 'x = "/languages" y = 1', cursor: 16 });
    expect(
      applySuggestion("a in (x)", { text: "y", start: 6, end: 7, kind: "value", detail: null })
        .text,
    ).toBe("a in (y)");
  });

  test("counts characters, not UTF-16 code units", () => {
    const next = applySuggestion("😀 = ", {
      text: '"x"',
      start: 4,
      end: 4,
      kind: "value",
      detail: null,
    });
    expect(next).toEqual({ text: '😀 = "x" ', cursor: 9 });
  });
});
