import { expect, test } from "vitest";
import { generate, written } from "../generate";

test("src/schema.ts is the types of openapi.json", async () => {
  expect(written(), "src/schema.ts is behind openapi.json; run `mise run api:generate`").toBe(
    await generate(),
  );
});
