import type { components } from "./schema";

const EVENTS_PATH = "/api/events";
const EVENTS_LOCK_NAME = "otelo-events";
const EVENTS_CHANNEL_NAME = "otelo-events";
const MIN_REOPEN_DELAY_MS = 5_000;
const MAX_REOPEN_DELAY_MS = 60_000;

type Indexing = components["schemas"]["Indexing"];

type TabMessage = IndexingMessage | IndexingRequestMessage;

interface IndexingMessage {
  readonly kind: "indexing";
  readonly indexing: Indexing;
}

interface IndexingRequestMessage {
  readonly kind: "indexing-request";
}

// A browser opens six connections to a host at most over HTTP/1.1, and an event stream holds
// one for good, so the tab that holds the lock reads the stream and tells the other tabs.
export function subscribeToIndexing(onIndexing: (indexing: Indexing) => void): () => void {
  const unsubscribed = new AbortController();
  // Web Locks exist only in a secure context, and without them each tab reads its own stream.
  if (!("locks" in navigator)) {
    void readEventStream(unsubscribed.signal, onIndexing);
    return () => unsubscribed.abort();
  }
  const channel = new BroadcastChannel(EVENTS_CHANNEL_NAME);
  let readIndexing: Indexing | undefined;
  let isReadingStream = false;
  const postToTabs = (message: TabMessage) =>
    // oxlint-disable-next-line unicorn/require-post-message-target-origin -- a BroadcastChannel stays in its origin
    channel.postMessage(message);
  channel.addEventListener("message", (event: MessageEvent<TabMessage>) => {
    const message = event.data;
    if (message.kind === "indexing") onIndexing(message.indexing);
    else if (isReadingStream && readIndexing)
      postToTabs({ kind: "indexing", indexing: readIndexing });
  });
  const readAndShareEventStream = async () => {
    isReadingStream = true;
    await readEventStream(unsubscribed.signal, (indexing) => {
      readIndexing = indexing;
      onIndexing(indexing);
      postToTabs({ kind: "indexing", indexing });
    });
    isReadingStream = false;
  };
  navigator.locks
    .request(EVENTS_LOCK_NAME, { signal: unsubscribed.signal }, readAndShareEventStream)
    .catch(() => undefined);
  postToTabs({ kind: "indexing-request" });
  return () => {
    unsubscribed.abort();
    channel.close();
  };
}

function readEventStream(
  unsubscribed: AbortSignal,
  onIndexing: (indexing: Indexing) => void,
): Promise<void> {
  return new Promise((resolve) => {
    if (unsubscribed.aborted) {
      resolve();
      return;
    }
    let source: EventSource | undefined;
    let reopenTimer: ReturnType<typeof setTimeout> | undefined;
    let reopenDelayMs = MIN_REOPEN_DELAY_MS;
    const openEventStream = () => {
      const openedSource = new EventSource(EVENTS_PATH);
      source = openedSource;
      openedSource.addEventListener("open", () => {
        reopenDelayMs = MIN_REOPEN_DELAY_MS;
      });
      openedSource.addEventListener("indexing", (event: MessageEvent<string>) => {
        // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- the spec names the type of the event
        onIndexing(JSON.parse(event.data) as Indexing);
      });
      // The browser reconnects after a dropped connection, and gives up after an answer that
      // is not a stream, such as the one of a proxy while the daemon restarts.
      openedSource.addEventListener("error", () => {
        if (openedSource.readyState !== EventSource.CLOSED) return;
        reopenTimer = setTimeout(openEventStream, reopenDelayMs);
        reopenDelayMs = Math.min(reopenDelayMs * 2, MAX_REOPEN_DELAY_MS);
      });
    };
    unsubscribed.addEventListener("abort", () => {
      clearTimeout(reopenTimer);
      source?.close();
      resolve();
    });
    openEventStream();
  });
}
