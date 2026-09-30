import { describe, expect, test } from "vitest";

import {
  addTerm,
  quoteString,
  writeAttributeField,
  writeLiteral,
  writeResourceField,
  writeTemplateTerm,
} from "../src/query";

describe("writeAttributeField", () => {
  test("writes a plain key as it is", () => {
    expect(writeAttributeField("http.route")).toBe("http.route");
    expect(writeAttributeField("město")).toBe("město");
  });

  test("prefixes a key that reads as something else", () => {
    expect(writeAttributeField("level")).toBe("attr.level");
    expect(writeAttributeField("duration")).toBe("attr.duration");
    expect(writeAttributeField("resource.x")).toBe("attr.resource.x");
    expect(writeAttributeField("OR")).toBe("attr.OR");
  });

  test("puts any other key in backticks", () => {
    expect(writeAttributeField("odd key")).toBe("`odd key`");
    expect(writeAttributeField("7up")).toBe("`7up`");
  });
});

test("writeResourceField names only the keys the lexer reads as one word", () => {
  expect(writeResourceField("host.name")).toBe("resource.host.name");
  expect(writeResourceField("odd key")).toBeUndefined();
});

test("quoteString escapes what the lexer unescapes", () => {
  expect(quoteString('say "hi"\\\n\t')).toBe('"say \\"hi\\"\\\\\\n\\t"');
});

test("writeLiteral writes scalars and skips the rest", () => {
  expect(writeLiteral("/matches")).toBe('"/matches"');
  expect(writeLiteral(200)).toBe("200");
  expect(writeLiteral(true)).toBe("true");
  expect(writeLiteral(null)).toBeUndefined();
  expect(writeLiteral([1])).toBeUndefined();
  expect(writeLiteral({ a: 1 })).toBeUndefined();
});

describe("addTerm", () => {
  test("joins with AND", () => {
    expect(addTerm("", "a = 1")).toBe("a = 1");
    expect(addTerm("  b = 2 ", "a = 1")).toBe("b = 2 a = 1");
  });

  test("keeps a query that ends in the term", () => {
    expect(addTerm("b = 2 a = 1", "a = 1")).toBe("b = 2 a = 1");
  });

  test("wraps a query with a top-level OR", () => {
    expect(addTerm("a = 1 or b = 2", "c = 3")).toBe("(a = 1 or b = 2) c = 3");
    expect(addTerm("(a = 1 OR b = 2)", "c = 3")).toBe("(a = 1 OR b = 2) c = 3");
    expect(addTerm('body ~ "this or that"', "c = 3")).toBe('body ~ "this or that" c = 3');
    expect(addTerm("color = 1", "c = 3")).toBe("color = 1 c = 3");
  });
});

test("writeTemplateTerm keeps the words without placeholders", () => {
  expect(writeTemplateTerm("payment <num> failed for user=<num> after <num>ms")).toBe(
    'body ~ "payment failed for after"',
  );
  expect(writeTemplateTerm("<num> <uuid>")).toBeUndefined();
});
