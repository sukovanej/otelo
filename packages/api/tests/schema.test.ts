import { expect, test } from "vitest";

import { generateSchema, readWrittenSchema } from "../generate";

test("src/schema.ts is the types of openapi.json", async () => {
  expect(
    readWrittenSchema(),
    "src/schema.ts is behind openapi.json; run `mise run api:generate`",
  ).toBe(await generateSchema());
});
