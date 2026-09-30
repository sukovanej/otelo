import { expect, test } from "vitest";

import { decodePathSegment } from "../src/path";

test("decodePathSegment decodes the escapes of encodeURIComponent", () => {
  expect(decodePathSegment("my%20api")).toBe("my api");
  expect(decodePathSegment(encodeURIComponent("billing/eu #2"))).toBe("billing/eu #2");
});

test("decodePathSegment keeps a segment with a malformed escape", () => {
  expect(decodePathSegment("100%")).toBe("100%");
  expect(decodePathSegment("api%E0%A4%A")).toBe("api%E0%A4%A");
});
