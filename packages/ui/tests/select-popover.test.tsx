import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes, mountView } from "@otelo/testing";

import { Select, type SelectOption } from "../src";

const LONG_OPTIONS: ReadonlyArray<SelectOption<string>> = [
  { value: "http.request.header.x-forwarded-for", label: "http.request.header.x-forwarded-for" },
  { value: "http.response.status_code", label: "http.response.status_code" },
  { value: "service.instance.id", label: "service.instance.id" },
];

test("a list that opens against the right edge stays inside the window while the search is typed", async () => {
  const field = page.getByRole("combobox", { name: "Group by" });
  const list = page.getByRole("listbox", { name: "Group by" });

  const { artifact } = await captureArtifact(async () => {
    mountView(() => (
      <div class="flex justify-end">
        <div class="w-32">
          <Select
            selection="multiple"
            label="Group by"
            options={LONG_OPTIONS}
            values={[]}
            typedValue={(text) => text}
            onChange={() => {}}
          />
        </div>
      </div>
    ));
    await userEvent.click(field);
    await expect.poll(() => readRightEdge(list.element())).toBeLessThanOrEqual(window.innerWidth);

    await userEvent.type(field, "http");
    await expect.element(list).toBeInTheDocument();
    expect(readRightEdge(list.element())).toBeLessThanOrEqual(window.innerWidth);
  });

  expectNoReactivityMistakes(artifact);
});

function readRightEdge(element: Element): number {
  return element.getBoundingClientRect().right;
}
