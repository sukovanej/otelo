import { expect, test } from "vitest";

import { SPAN_GROUPINGS, writeGroupFilter } from "../src/services/span-grouping";

test("a group adds a term for each of its values and rules out the ones it lacks", () => {
  const { by, writeFilter } = SPAN_GROUPINGS.query;
  const group = {
    values: { "db.system.name": "sqlite", "db.query.text": "SELECT *\nFROM users" },
    name: "SELECT",
    attributes: {},
    spans: { count: 1, errors: 0, total_ns: 1, latency: null },
  };
  expect(writeGroupFilter(writeFilter("api"), by, group)).toBe(
    'service = "api" has(db.system.name) db.system.name = "sqlite" NOT has(db.namespace) ' +
      'db.query.text = "SELECT *\\nFROM users"',
  );
});
