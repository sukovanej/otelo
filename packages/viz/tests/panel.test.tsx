import { captureArtifact } from "@solidjs/diagnostics";
import { expect, test } from "vitest";
import { page } from "vitest/browser";

import { expectNoReactivityMistakes, mountView } from "@otelo/testing";

import { Panel } from "../src";

interface DrawnPartProps {
  readonly name: string;
}

test("a panel draws its actions and its description once", async () => {
  const drawn: string[] = [];
  function DrawnPart(props: DrawnPartProps) {
    drawn.push(props.name);
    return <span>{props.name}</span>;
  }

  const { artifact } = await captureArtifact(async () => {
    mountView(() => (
      <Panel
        title="Latency"
        description={<DrawnPart name="p99 by route" />}
        actions={<DrawnPart name="Refresh" />}
      >
        Body
      </Panel>
    ));
    await expect.element(page.getByText("Refresh")).toBeInTheDocument();
  });

  expect(drawn.toSorted()).toEqual(["Refresh", "p99 by route"]);
  expectNoReactivityMistakes(artifact);
});
