import { useQueryClient } from "@tanstack/solid-query";
import { createMemo, createSignal, onSettled, Show, useContext } from "solid-js";

import type { Indexing, SignalIndexing } from "@otelo/api";

import { ApiContext } from "./api";

const INDEXED_SIGNALS = ["logs", "spans", "metrics"] as const satisfies ReadonlyArray<
  keyof Indexing
>;

export default function IndexingProgressBar() {
  const api = useContext(ApiContext);
  const queryClient = useQueryClient();
  const [indexing, setIndexing] = createSignal<Indexing>();
  const showIndexing = (nextIndexing: Indexing) => {
    const shownIndexing = indexing();
    if (shownIndexing && hasAnySignalCaughtUp(shownIndexing, nextIndexing))
      void queryClient.invalidateQueries();
    setIndexing(nextIndexing);
  };
  onSettled(() => api.subscribeToIndexing(showIndexing));
  const isCatchingUp = () =>
    INDEXED_SIGNALS.some((signal) => indexing()?.[signal].state === "catching_up");
  const indexedPercent = createMemo(() => {
    const shownIndexing = indexing();
    if (!shownIndexing) return 0;
    const percentSum = INDEXED_SIGNALS.reduce(
      (sum, signal) => sum + measureIndexedPercent(shownIndexing[signal]),
      0,
    );
    return Math.round(percentSum / INDEXED_SIGNALS.length);
  });

  return (
    <Show when={isCatchingUp()}>
      <div class="shrink-0 border-b border-line bg-surface">
        <div class="flex items-center gap-3 px-4 py-1.5 text-muted">
          <span class="min-w-0">
            Indexing the journal. Pages show the newest telemetry once it finishes.
          </span>
          <span class="ml-auto shrink-0 whitespace-nowrap text-ink tabular-nums">
            {indexedPercent()} %
          </span>
        </div>
        <div
          role="progressbar"
          aria-label="Indexing the journal"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={indexedPercent()}
          class="h-1 bg-line"
        >
          <div
            class="h-full bg-accent transition-[width] duration-1000 ease-linear"
            style={{ width: `${indexedPercent()}%` }}
          />
        </div>
      </div>
    </Show>
  );
}

function hasAnySignalCaughtUp(shownIndexing: Indexing, nextIndexing: Indexing): boolean {
  return INDEXED_SIGNALS.some(
    (signal) =>
      shownIndexing[signal].state === "catching_up" && nextIndexing[signal].state === "caught_up",
  );
}

function measureIndexedPercent(signalIndexing: SignalIndexing): number {
  if (signalIndexing.state === "catching_up") return signalIndexing.indexed_percent;
  return signalIndexing.state === "caught_up" ? 100 : 0;
}
