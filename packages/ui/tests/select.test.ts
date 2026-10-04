import { expect, test } from "vitest";

import { groupSelectOptions } from "../src/Select/select-options";

const OPTIONS = [
  { value: "service", label: "service", section: "Fields" },
  { value: "http.route", label: "http.route", section: "Attributes" },
  { value: "resource.host.name", label: "resource.host.name", section: "Resource" },
  { value: "http.request.method", label: "http.request.method", section: "Attributes" },
];

test("options keep their sections in order, and the search keeps the ones with every word", () => {
  expect(groupSelectOptions(OPTIONS, "").map((group) => group.title)).toEqual([
    "Fields",
    "Attributes",
    "Resource",
  ]);
  expect(groupSelectOptions(OPTIONS, "HTTP method")).toEqual([
    { title: "Attributes", options: [OPTIONS[3]] },
  ]);
  expect(groupSelectOptions([{ value: "a", label: "A" }], "")).toEqual([
    { title: "", options: [{ value: "a", label: "A" }] },
  ]);
});
