import { afterEach, beforeEach, expect, test, vi } from "vitest";

import type { Indexing } from "../src";
import { subscribeToIndexing } from "../src/events";

const CAUGHT_UP: Indexing = {
  logs: { state: "caught_up" },
  spans: { state: "caught_up" },
  metrics: { state: "caught_up" },
};

class FakeEventSource {
  static readonly CLOSED = 2;
  static readonly opened: FakeEventSource[] = [];
  readyState = 1;
  private readonly listeners = new Map<string, (event: MessageEvent<string>) => void>();

  constructor(readonly url: string) {
    FakeEventSource.opened.push(this);
  }

  addEventListener(type: string, listener: (event: MessageEvent<string>) => void) {
    this.listeners.set(type, listener);
  }

  close() {
    this.readyState = FakeEventSource.CLOSED;
  }

  sendIndexing(indexing: Indexing) {
    this.listeners.get("indexing")?.(
      new MessageEvent("indexing", { data: JSON.stringify(indexing) }),
    );
  }

  refuse() {
    this.readyState = FakeEventSource.CLOSED;
    this.listeners.get("error")?.(new MessageEvent("error"));
  }
}

const unsubscribes: Array<() => void> = [];

beforeEach(() => {
  FakeEventSource.opened.length = 0;
  vi.stubGlobal("EventSource", FakeEventSource);
});

afterEach(() => {
  for (const unsubscribe of unsubscribes.splice(0)) unsubscribe();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

function subscribeAsTab(): Indexing[] {
  const received: Indexing[] = [];
  unsubscribes.push(subscribeToIndexing((indexing) => received.push(indexing)));
  return received;
}

function readOpenStreams(): FakeEventSource[] {
  return FakeEventSource.opened.filter((source) => source.readyState !== FakeEventSource.CLOSED);
}

test("one tab reads the stream, and every tab hears its events", async () => {
  const firstTab = subscribeAsTab();
  const secondTab = subscribeAsTab();
  await vi.waitFor(() => expect(readOpenStreams()).toHaveLength(1));

  readOpenStreams()[0]?.sendIndexing(CAUGHT_UP);

  await vi.waitFor(() => expect(secondTab).toEqual([CAUGHT_UP]));
  expect(firstTab).toEqual([CAUGHT_UP]);
  expect(FakeEventSource.opened).toHaveLength(1);
  expect(FakeEventSource.opened[0]?.url).toBe("/api/events");
});

test("a tab that opens later hears the last event at once", async () => {
  subscribeAsTab();
  await vi.waitFor(() => expect(readOpenStreams()).toHaveLength(1));
  readOpenStreams()[0]?.sendIndexing(CAUGHT_UP);

  const laterTab = subscribeAsTab();

  await vi.waitFor(() => expect(laterTab).toEqual([CAUGHT_UP]));
});

test("another tab reads the stream once the reading tab closes", async () => {
  subscribeAsTab();
  const secondTab = subscribeAsTab();
  await vi.waitFor(() => expect(readOpenStreams()).toHaveLength(1));

  unsubscribes.shift()?.();

  await vi.waitFor(() => expect(FakeEventSource.opened).toHaveLength(2));
  expect(readOpenStreams()).toEqual([FakeEventSource.opened[1]]);
  readOpenStreams()[0]?.sendIndexing(CAUGHT_UP);
  expect(secondTab).toEqual([CAUGHT_UP]);
});

test("the reading tab opens a refused stream again, waiting twice as long each time", async () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  subscribeAsTab();
  const secondTab = subscribeAsTab();
  await vi.waitFor(() => expect(readOpenStreams()).toHaveLength(1));

  readOpenStreams()[0]?.refuse();
  vi.advanceTimersByTime(5_000);
  expect(readOpenStreams()).toHaveLength(1);
  readOpenStreams()[0]?.refuse();
  vi.advanceTimersByTime(5_000);
  expect(readOpenStreams()).toHaveLength(0);
  vi.advanceTimersByTime(5_000);

  expect(readOpenStreams()).toHaveLength(1);
  readOpenStreams()[0]?.sendIndexing(CAUGHT_UP);
  await vi.waitFor(() => expect(secondTab).toEqual([CAUGHT_UP]));
});

test("without Web Locks each tab reads its own stream and hears only it", async () => {
  vi.stubGlobal("navigator", {});
  const firstTab = subscribeAsTab();
  const secondTab = subscribeAsTab();
  expect(readOpenStreams()).toHaveLength(2);

  readOpenStreams()[0]?.sendIndexing(CAUGHT_UP);
  await new Promise((resolve) => setTimeout(resolve, 50));

  expect(firstTab).toEqual([CAUGHT_UP]);
  expect(secondTab).toEqual([]);
});
