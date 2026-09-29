import { expect, test } from "vitest";

import {
  databaseId,
  type DbSpan,
  dbTitle,
  language,
  nameRest,
  routeParts,
  spanMeaning,
} from "../src/semantics";

test("an HTTP server span has its method, route, and status", () => {
  expect(
    spanMeaning({
      "http.request.method": "get",
      "http.route": "/api/traces/{trace_id}",
      "url.path": "/api/traces/4ba2",
      "http.response.status_code": 200,
    }),
  ).toEqual({ type: "http", method: "GET", route: "/api/traces/{trace_id}", status: 200 });
});

test("an HTTP client span takes the path of its URL, and the old names count", () => {
  expect(
    spanMeaning({
      "http.method": "POST",
      "http.url": "https://api.stripe.com/v1/charges?x=1",
      "http.status_code": "402",
    }),
  ).toEqual({ type: "http", method: "POST", route: "/v1/charges", status: 402 });
});

test("a database span has its system and its query as it is", () => {
  expect(
    spanMeaning({
      "db.system.name": "sqlite",
      "db.query.text": "  SELECT l.ts,\n   r.service FROM logs",
    }),
  ).toEqual({
    type: "db",
    system: "sqlite",
    query: "  SELECT l.ts,\n   r.service FROM logs",
  });
  expect(spanMeaning({ "db.system": "redis", "db.statement": "HGET k f" })).toEqual({
    type: "db",
    system: "redis",
    query: "HGET k f",
  });
});

const dbSpan = (query?: string): DbSpan => ({ type: "db", system: "sqlite", query });

test("dbTitle puts the first word of the query or the name on a badge", () => {
  expect(dbTitle(dbSpan("SELECT *\n  FROM users"), "SELECT")).toEqual({
    keyword: "SELECT",
    rest: "*\n  FROM users",
    text: "SELECT *\n  FROM users",
  });
  expect(dbTitle(dbSpan(), "SELECT users")).toMatchObject({ keyword: "SELECT", rest: "users" });
  expect(dbTitle(dbSpan(), "SELECT")).toMatchObject({ keyword: "SELECT", rest: "" });
  // A first word that is not letters alone stays in the text.
  expect(dbTitle(dbSpan("(SELECT 1) UNION SELECT 2"), "x")).toMatchObject({ keyword: undefined });
  expect(dbTitle(dbSpan(), "users.find")).toMatchObject({ keyword: undefined, rest: "users.find" });
});

test("a span without either is other", () => {
  expect(spanMeaning({ "code.function": "open" })).toEqual({ type: "other" });
});

test("nameRest drops what the badges of an HTTP span say", () => {
  const http = spanMeaning({ "http.request.method": "GET", "http.route": "/users" });
  expect(nameRest("GET /users", http)).toBe("");
  expect(nameRest("GET", http)).toBe("");
  expect(nameRest("list users", http)).toBe("list users");
  const db = spanMeaning({ "db.system": "postgresql", "db.query.text": "SELECT * FROM carts" });
  expect(nameRest("SELECT carts", db)).toBe("SELECT carts");
});

test("language reads the SDK language of a resource", () => {
  expect(language({ "telemetry.sdk.language": "rust" })).toBe("rust");
  expect(language({ "telemetry.sdk.language": "nodejs" })).toBe("javascript");
  expect(language({ "telemetry.sdk.language": "dotnet" })).toBe("csharp");
  expect(language({ "service.name": "api" })).toBeUndefined();
});

test("databaseId names a system the way the database icons do", () => {
  expect(databaseId("postgresql")).toBe("postgresql");
  expect(databaseId("microsoft.sql_server")).toBe("mssql");
  expect(databaseId("oracle.db")).toBe("oracle");
});

const parts = (route: string) => routeParts(route).map((p) => `${p.kind}:${p.text}`);

test("routeParts marks the slashes and the parameters of a route", () => {
  expect(parts("/users/{id}/orders")).toEqual([
    "slash:/",
    "text:users",
    "slash:/",
    "param:{id}",
    "slash:/",
    "text:orders",
  ]);
  expect(parts("/a/:name/<file>/*")).toEqual([
    "slash:/",
    "text:a",
    "slash:/",
    "param::name",
    "slash:/",
    "param:<file>",
    "slash:/",
    "param:*",
  ]);
  // A colon inside a segment is text.
  expect(parts("/at/10:30")).toEqual(["slash:/", "text:at", "slash:/", "text:10:30"]);
  expect(parts("/users:batch")).toEqual(["slash:/", "text:users:batch"]);
  expect(parts("")).toEqual([]);
});
