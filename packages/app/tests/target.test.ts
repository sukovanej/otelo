import { expect, test } from "vitest";

import {
  applySummaryToAttributes,
  nameTargetSystem,
  parseTarget,
  toTargetLabel,
  toTargetParams,
} from "../src/services/target";

const postgres = { type: "database", system: "postgresql", name: "app" } as const;
const stripe = { type: "http", system: null, name: "api.stripe.com" } as const;

test("a target names its system and what it calls", () => {
  expect(nameTargetSystem(postgres)).toBe("PostgreSQL");
  expect(toTargetLabel(postgres)).toBe("PostgreSQL app");
  expect(toTargetLabel({ ...postgres, system: "microsoft.sql_server", name: null })).toBe(
    "SQL Server",
  );
  expect(toTargetLabel(stripe)).toBe("api.stripe.com");
  expect(toTargetLabel({ type: "http", system: null, name: null })).toBe("HTTP");
  expect(toTargetLabel({ type: "messaging", system: "kafka", name: "orders" })).toBe(
    "kafka orders",
  );
});

test("a target goes to the API without the parts it lacks", () => {
  expect(toTargetParams(postgres)).toEqual({
    type: "database",
    system: "postgresql",
    target: "app",
  });
  expect(toTargetParams(stripe)).toEqual({ type: "http", target: "api.stripe.com" });
});

test("a target comes back from the URL of a known type only", () => {
  expect(parseTarget("http", undefined, "api.stripe.com")).toEqual(stripe);
  expect(parseTarget("toString", undefined, undefined)).toBeUndefined();
  expect(parseTarget(undefined, "postgresql", "app")).toBeUndefined();
});

test("a summary shows as the query or the route of a call", () => {
  const one = { "db.query.text": "SELECT 1", "http.url": "https://x/users/7" };
  expect(applySummaryToAttributes("database", "SELECT ?", one)["db.query.text"]).toBe("SELECT ?");
  expect(applySummaryToAttributes("http", "GET /users/{id}", one)["http.route"]).toBe(
    "/users/{id}",
  );
  expect(applySummaryToAttributes("http", "GET", one)).toBe(one);
  expect(applySummaryToAttributes("rpc", "Charges/Create", one)).toBe(one);
});
