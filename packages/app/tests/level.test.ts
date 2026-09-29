import { expect, test } from "vitest";

import { levelName } from "../src/logs/level";

test("levelName names each band of four severity numbers", () => {
  expect([1, 4, 5, 9, 13, 16, 17, 21, 24].map(levelName)).toEqual([
    "TRACE",
    "TRACE",
    "DEBUG",
    "INFO",
    "WARN",
    "WARN",
    "ERROR",
    "FATAL",
    "FATAL",
  ]);
  expect([0, 25, -1].map(levelName)).toEqual(["UNSPECIFIED", "UNSPECIFIED", "UNSPECIFIED"]);
});
