import { captureArtifact } from "@solidjs/diagnostics";
import { createSignal } from "solid-js";
import { expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";

import { expectNoReactivityMistakes, mountView } from "@otelo/testing";

import { ChartPanel, type TimeFrame, type TimeSeries } from "../src";

const FRAME: TimeFrame = {
  bucketStartsMs: [0, 60_000, 120_000],
  stepMs: 60_000,
  startMs: 0,
  endMs: 180_000,
};

test("a reload with the same series keeps the legend and the table it drew", async () => {
  const [series, setSeries] = createSignal(readLatencySeries());
  const legendButton = page.getByRole("button", { name: "p99" });

  const { artifact } = await captureArtifact(async () => {
    mountView(() => (
      <ChartPanel
        title="Latency"
        description=""
        frame={FRAME}
        series={series()}
        kind="line"
        unit="duration"
        loading={false}
        onZoom={() => {}}
      />
    ));
    await userEvent.click(legendButton);
    const clickedButton = legendButton.element();
    const seriesHeader = page.getByRole("columnheader", { name: "p99" }).element();

    setSeries(readLatencySeries());
    await expect.element(legendButton).toHaveAttribute("aria-pressed", "true");

    expect(legendButton.element()).toBe(clickedButton);
    expect(document.activeElement).toBe(clickedButton);
    expect(page.getByRole("columnheader", { name: "p99" }).element()).toBe(seriesHeader);
  });

  expectNoReactivityMistakes(artifact);
});

function readLatencySeries(): ReadonlyArray<TimeSeries> {
  return [
    { label: "p50", values: [12e6, 14e6, 11e6] },
    { label: "p99", values: [80e6, 95e6, 70e6] },
  ];
}
