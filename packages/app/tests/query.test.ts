import { describe, expect, test } from "vitest";
import { addTerm, attributeField, literal, quote, resourceField, templateTerm } from "../src/query";

describe("attributeField", () => {
  test("writes a plain key as it is", () => {
    expect(attributeField("http.route")).toBe("http.route");
    expect(attributeField("město")).toBe("město");
  });

  test("prefixes a key that reads as something else", () => {
    expect(attributeField("level")).toBe("attr.level");
    expect(attributeField("duration")).toBe("attr.duration");
    expect(attributeField("resource.x")).toBe("attr.resource.x");
    expect(attributeField("OR")).toBe("attr.OR");
  });

  test("puts any other key in backticks", () => {
    expect(attributeField("odd key")).toBe("`odd key`");
    expect(attributeField("7up")).toBe("`7up`");
  });
});

test("resourceField names only the keys the lexer reads as one word", () => {
  expect(resourceField("host.name")).toBe("resource.host.name");
  expect(resourceField("odd key")).toBeUndefined();
});

test("quote escapes what the lexer unescapes", () => {
  expect(quote('say "hi"\\\n\t')).toBe('"say \\"hi\\"\\\\\\n\\t"');
});

test("literal writes scalars and skips the rest", () => {
  expect(literal("/matches")).toBe('"/matches"');
  expect(literal(200)).toBe("200");
  expect(literal(true)).toBe("true");
  expect(literal(null)).toBeUndefined();
  expect(literal([1])).toBeUndefined();
  expect(literal({ a: 1 })).toBeUndefined();
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

test("templateTerm keeps the words without placeholders", () => {
  expect(templateTerm("payment <num> failed for user=<num> after <num>ms")).toBe(
    'body ~ "payment failed for after"',
  );
  expect(templateTerm("<num> <uuid>")).toBeUndefined();
});
