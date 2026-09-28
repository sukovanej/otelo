import { expect, test } from "vitest";
import { databaseId, language, nameRest, spanMeaning } from "../src/semantics";

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

test("a database span has its system and the keyword of its query", () => {
  expect(
    spanMeaning({
      "db.system.name": "sqlite",
      "db.query.text": "  SELECT l.ts,\n   r.service FROM logs",
    }),
  ).toEqual({
    type: "db",
    system: "sqlite",
    operation: "SELECT",
    query: "SELECT l.ts, r.service FROM logs",
  });
  expect(spanMeaning({ "db.system": "redis", "db.operation": "hget" })).toEqual({
    type: "db",
    system: "redis",
    operation: "HGET",
    query: undefined,
  });
});

test("a span without either is other", () => {
  expect(spanMeaning({ "code.function": "open" })).toEqual({ type: "other" });
});

test("nameRest drops what the badge says", () => {
  const http = spanMeaning({ "http.request.method": "GET", "http.route": "/users" });
  expect(nameRest("GET /users", http)).toBe("");
  expect(nameRest("GET", http)).toBe("");
  expect(nameRest("list users", http)).toBe("list users");
  const db = spanMeaning({ "db.system": "postgresql", "db.operation.name": "SELECT" });
  expect(nameRest("SELECT carts", db)).toBe("carts");
  expect(nameRest("SELECT", db)).toBe("");
  expect(nameRest("SELECTIVE cache", db)).toBe("SELECTIVE cache");
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
