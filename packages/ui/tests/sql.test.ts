import { expect, test } from "vitest";

import { layOutSql, lexSql } from "../src/sql";

test("lexSql tells keywords, names, strings, numbers, and punctuation apart", () => {
  expect(listTokenKinds(`select "a", u.name FROM users u WHERE n >= 12.5 AND s = 'it''s'`)).toEqual(
    [
      "keyword:select",
      'text:"a"',
      "punctuation:,",
      "text:u",
      "punctuation:.",
      "text:name",
      "keyword:FROM",
      "text:users",
      "text:u",
      "keyword:WHERE",
      "text:n",
      "text:>",
      "text:=",
      "number:12.5",
      "keyword:AND",
      "text:s",
      "text:=",
      "string:'it''s'",
    ],
  );
});

test("lexSql knows the placeholders of each driver, and a cast from one", () => {
  expect(listTokenKinds("? $1 :id @p1 %s %(name)s x::text")).toEqual([
    "placeholder:?",
    "placeholder:$1",
    "placeholder::id",
    "placeholder:@p1",
    "placeholder:%s",
    "placeholder:%(name)s",
    "text:x",
    "text:::",
    "text:text",
  ]);
});

test("lexSql reads comments, and what lacks its end runs to the end", () => {
  expect(listTokenKinds("SELECT 1 -- one\n/* two */ FROM t")).toEqual([
    "keyword:SELECT",
    "number:1",
    "comment:-- one",
    "comment:/* two */",
    "keyword:FROM",
    "text:t",
  ]);
  expect(listTokenKinds("x = 'open AND")).toEqual(["text:x", "text:=", "string:'open AND"]);
  expect(listTokenKinds("/* open SELECT")).toEqual(["comment:/* open SELECT"]);
});

test("lexSql join back into the text", () => {
  const sql = "  SELECT l.ts,\n   r.service FROM logs -- x\n WHERE a=$1;";
  expect(
    lexSql(sql)
      .map((token) => token.text)
      .join(""),
  ).toBe(sql);
  expect(lexSql("")).toEqual([]);
});

test("layOutSql starts a line at each clause, and at each condition a level deeper", () => {
  expect(
    printLayout(
      "select l.ts,r.service from logs l left outer join resources r on r.id = l.resource_id " +
        "where l.ts > ? and r.service = $1 or l.n between 1 and 2 " +
        "group by 1 having count(*) > 1 order by l.ts desc limit 50 offset 10",
    ),
  ).toBe(
    [
      "select l.ts, r.service",
      "from logs l",
      "left outer join resources r on r.id = l.resource_id",
      "where l.ts > ?",
      "  and r.service = $1",
      "  or l.n between 1 and 2",
      "group by 1",
      "having count(*) > 1",
      "order by l.ts desc",
      "limit 50",
      "offset 10",
    ].join("\n"),
  );
});

test("layOutSql takes the lines and spaces a query came with for one space", () => {
  expect(printLayout("  SELECT *\n\n    FROM   users\n  WHERE id = ?  ")).toBe(
    "SELECT *\nFROM users\nWHERE id = ?",
  );
});

test("layOutSql puts a query in parentheses a level deeper than the line that opens it", () => {
  expect(
    printLayout(
      "WITH recent AS (SELECT id FROM orders WHERE ts > ?), big AS (SELECT 1) " +
        "SELECT * FROM recent WHERE a = 1 AND id IN (SELECT id FROM big) AND b = 2 ORDER BY id",
    ),
  ).toBe(
    [
      "WITH recent AS (",
      "  SELECT id",
      "  FROM orders",
      "  WHERE ts > ?",
      "), big AS (",
      "  SELECT 1",
      ")",
      "SELECT *",
      "FROM recent",
      "WHERE a = 1",
      "  AND id IN (",
      "    SELECT id",
      "    FROM big",
      "  )",
      "  AND b = 2",
      "ORDER BY id",
    ].join("\n"),
  );
});

test("layOutSql keeps what other parentheses and a CASE hold on their line", () => {
  expect(
    printLayout(
      "SELECT extract(year FROM ts), count(*) FILTER (WHERE ok AND fast), " +
        "CASE WHEN a AND b THEN 1 ELSE 0 END, rank() OVER (PARTITION BY a ORDER BY b) FROM t",
    ),
  ).toBe(
    [
      "SELECT extract(year FROM ts), count(*) FILTER (WHERE ok AND fast), " +
        "CASE WHEN a AND b THEN 1 ELSE 0 END, rank() OVER (PARTITION BY a ORDER BY b)",
      "FROM t",
    ].join("\n"),
  );
});

test("layOutSql lays out the statements that write", () => {
  expect(
    printLayout(
      "INSERT INTO users (id, name) VALUES (?, ?) ON CONFLICT (id) DO UPDATE SET name = excluded.name RETURNING id",
    ),
  ).toBe(
    [
      "INSERT INTO users (id, name)",
      "VALUES (?, ?)",
      "ON CONFLICT (id) DO UPDATE",
      "SET name = excluded.name",
      "RETURNING id",
    ].join("\n"),
  );
  expect(printLayout("UPDATE users SET a = 1, b = 2 WHERE id = ?")).toBe(
    "UPDATE users\nSET a = 1, b = 2\nWHERE id = ?",
  );
  expect(printLayout("DELETE FROM users WHERE id = ?")).toBe("DELETE FROM users\nWHERE id = ?");
  expect(printLayout("SELECT * FROM jobs WHERE done = 0 FOR UPDATE SKIP LOCKED")).toBe(
    "SELECT *\nFROM jobs\nWHERE done = 0\nFOR UPDATE SKIP LOCKED",
  );
});

test("layOutSql starts a line after a statement and after a line comment", () => {
  expect(printLayout("BEGIN; SELECT 1 -- one\n, 2; COMMIT")).toBe(
    "BEGIN;\nSELECT 1 -- one\n, 2;\nCOMMIT",
  );
});

test("layOutSql lays out what is not SQL without failing", () => {
  expect(printLayout("users.find")).toBe("users.find");
  expect(printLayout(")) AND (")).toBe("))\n  AND (");
  expect(layOutSql("")).toEqual([]);
});

function listTokenKinds(sql: string): string[] {
  return lexSql(sql)
    .filter((token) => token.kind !== "space")
    .map((token) => `${token.kind}:${token.text}`);
}

function printLayout(sql: string): string {
  return layOutSql(sql)
    .map((line) => "  ".repeat(line.indentLevel) + line.tokens.map((token) => token.text).join(""))
    .join("\n");
}
