import { expect, test } from "vitest";

import {
  isSqlQueryAttribute,
  isSqlSystem,
  readLanguageIconId,
  readSpanMeaning,
  splitRouteIntoParts,
  stripBadgesFromName,
  toDatabaseIconId,
} from "../src/semantics";

test("an HTTP server span has its method, route, and status", () => {
  expect(
    readSpanMeaning({
      "http.request.method": "get",
      "http.route": "/api/traces/{trace_id}",
      "url.path": "/api/traces/4ba2",
      "http.response.status_code": 200,
    }),
  ).toEqual({ kind: "http", method: "GET", route: "/api/traces/{trace_id}", status: 200 });
});

test("an HTTP client span takes the path of its URL, and the old names count", () => {
  expect(
    readSpanMeaning({
      "http.method": "POST",
      "http.url": "https://api.stripe.com/v1/charges?x=1",
      "http.status_code": "402",
    }),
  ).toEqual({ kind: "http", method: "POST", route: "/v1/charges", status: 402 });
});

test("a database span has its system and its query as it is", () => {
  expect(
    readSpanMeaning({
      "db.system.name": "sqlite",
      "db.query.text": "  SELECT log.logged_at,\n   resource.service FROM logs",
    }),
  ).toEqual({
    kind: "database",
    system: "sqlite",
    query: "  SELECT log.logged_at,\n   resource.service FROM logs",
  });
  expect(readSpanMeaning({ "db.system": "redis", "db.statement": "HGET k f" })).toEqual({
    kind: "database",
    system: "redis",
    query: "HGET k f",
  });
});

test("isSqlSystem tells the systems that speak SQL, by their new names and their old ones", () => {
  for (const system of ["postgresql", "sqlite", "microsoft.sql_server", "mssql", "other_sql"]) {
    expect(isSqlSystem(system)).toBe(true);
  }
  for (const system of ["redis", "mongodb", "elasticsearch", "cassandra", "aws.dynamodb"]) {
    expect(isSqlSystem(system)).toBe(false);
  }
});

test("isSqlQueryAttribute is the query of a call to a system that speaks SQL", () => {
  const sql = {
    "db.system.name": "postgresql",
    "db.query.text": "SELECT 1",
    "db.namespace": "app",
  };
  expect(isSqlQueryAttribute(sql, "db.query.text")).toBe(true);
  expect(isSqlQueryAttribute(sql, "db.namespace")).toBe(false);
  expect(
    isSqlQueryAttribute({ "db.system": "mysql", "db.statement": "SELECT 1" }, "db.statement"),
  ).toBe(true);
  expect(
    isSqlQueryAttribute({ "db.system.name": "redis", "db.query.text": "GET k" }, "db.query.text"),
  ).toBe(false);
  expect(isSqlQueryAttribute({ "db.query.text": "SELECT 1" }, "db.query.text")).toBe(false);
});

test("a span without either is other", () => {
  expect(readSpanMeaning({ "code.function": "open" })).toEqual({ kind: "other" });
});

test("stripBadgesFromName drops what the badges of an HTTP span say", () => {
  const http = readSpanMeaning({ "http.request.method": "GET", "http.route": "/users" });
  expect(stripBadgesFromName("GET /users", http)).toBe("");
  expect(stripBadgesFromName("GET", http)).toBe("");
  expect(stripBadgesFromName("list users", http)).toBe("list users");
  const database = readSpanMeaning({
    "db.system": "postgresql",
    "db.query.text": "SELECT * FROM carts",
  });
  expect(stripBadgesFromName("SELECT carts", database)).toBe("SELECT carts");
});

test("readLanguageIconId reads the SDK language of a resource", () => {
  expect(readLanguageIconId({ "telemetry.sdk.language": "rust" })).toBe("rust");
  expect(readLanguageIconId({ "telemetry.sdk.language": "nodejs" })).toBe("javascript");
  expect(readLanguageIconId({ "telemetry.sdk.language": "dotnet" })).toBe("csharp");
  expect(readLanguageIconId({ "service.name": "api" })).toBeUndefined();
});

test("toDatabaseIconId names a system the way the database icons do", () => {
  expect(toDatabaseIconId("postgresql")).toBe("postgresql");
  expect(toDatabaseIconId("microsoft.sql_server")).toBe("mssql");
  expect(toDatabaseIconId("oracle.db")).toBe("oracle");
});

test("splitRouteIntoParts marks the slashes and the parameters of a route", () => {
  expect(printRouteParts("/users/{id}/orders")).toEqual([
    "slash:/",
    "text:users",
    "slash:/",
    "parameter:{id}",
    "slash:/",
    "text:orders",
  ]);
  expect(printRouteParts("/a/:name/<file>/*")).toEqual([
    "slash:/",
    "text:a",
    "slash:/",
    "parameter::name",
    "slash:/",
    "parameter:<file>",
    "slash:/",
    "parameter:*",
  ]);
  expect(printRouteParts("")).toEqual([]);
});

test("a colon inside a segment is text", () => {
  expect(printRouteParts("/at/10:30")).toEqual(["slash:/", "text:at", "slash:/", "text:10:30"]);
  expect(printRouteParts("/users:batch")).toEqual(["slash:/", "text:users:batch"]);
});

function printRouteParts(route: string): string[] {
  return splitRouteIntoParts(route).map((part) => `${part.kind}:${part.text}`);
}
