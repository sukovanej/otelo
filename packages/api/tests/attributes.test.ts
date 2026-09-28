import { expectTypeOf, test } from "vitest";

import type { AttributeValue } from "../src";

test("an attribute value holds arrays and maps of attribute values", () => {
  expectTypeOf({ tags: ["a", 1, [true, { deep: null }]] }).toExtend<AttributeValue>();
  expectTypeOf<AttributeValue[]>().toExtend<AttributeValue>();
  expectTypeOf<{ f: () => void }>().not.toExtend<AttributeValue>();
  expectTypeOf<unknown[]>().not.toExtend<AttributeValue>();
});
