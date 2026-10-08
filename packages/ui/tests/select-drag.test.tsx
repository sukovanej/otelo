import { captureArtifact, expectRerunBudget } from "@solidjs/diagnostics";
import { expect, test } from "vitest";

import {
  expectNoReactivityMistakes,
  mountView,
  movePointerTo,
  type PointerPoint,
  pressPointerAt,
  readCenter,
  releasePointer,
} from "@otelo/testing";

import { Select, type SelectOption } from "../src";

const OPTIONS: ReadonlyArray<SelectOption<string>> = [
  { value: "service", label: "service" },
  { value: "route", label: "route" },
  { value: "status", label: "status" },
];

const CHOSEN = ["service", "route", "status"];

test("dragging a tag within one slot moves only its ghost", async () => {
  mountGroupBySelect(() => {});
  const grabbed = readTagCenter("service");
  await pressPointerAt(grabbed);
  await movePointerTo({ x: grabbed.x + 8, y: grabbed.y });

  const { artifact } = await captureArtifact(async () => {
    await movePointerTo({ x: grabbed.x + 9, y: grabbed.y });
    await movePointerTo({ x: grabbed.x + 10, y: grabbed.y });
    await movePointerTo({ x: grabbed.x + 11, y: grabbed.y });
  });
  await releasePointer();

  expectRerunBudget(artifact, 3);
  expectNoReactivityMistakes(artifact);
});

test("dropping a tag past the last one moves it to the end", async () => {
  const orders: string[][] = [];
  mountGroupBySelect((values) => orders.push(values));

  const { artifact } = await captureArtifact(async () => {
    const grabbed = readTagCenter("service");
    const last = readTagCenter("status");
    await pressPointerAt(grabbed);
    await movePointerTo({ x: grabbed.x + 8, y: grabbed.y });
    await movePointerTo({ x: last.x + 40, y: last.y });
    await releasePointer();
  });

  expect(orders).toEqual([["route", "status", "service"]]);
  expectNoReactivityMistakes(artifact);
});

function mountGroupBySelect(onChange: (values: string[]) => void) {
  mountView(() => (
    <div class="w-96">
      <Select
        selection="multiple"
        label="Group by"
        options={OPTIONS}
        values={CHOSEN}
        onChange={onChange}
      />
    </div>
  ));
}

function readTagCenter(value: string): PointerPoint {
  const tag = document.querySelector(`[data-reorder-value='${value}']`);
  if (!tag) throw new Error(`The select shows no ${value} tag`);
  return readCenter(tag);
}
