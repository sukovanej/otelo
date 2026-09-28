// The range of the services pages, and live mode, in the URL, so a link
// opens the same page and the range goes with a link from one page to the
// other.

import { useSearchParams } from "@solidjs/router";
import { type Accessor, createEffect, createMemo, onCleanup } from "solid-js";

import { search } from "@siner/api";

import { createFetch, type Fetched } from "../fetch";
import { DEFAULT_SINCE, LIVE_MS } from "../list";

export interface Range {
  since: () => string;
  until: () => string;
  /** Whether the page reloads by itself, which needs a range that ends now. */
  live: () => boolean;
  setRange: (since: string, until: string) => void;
  setLive: (live: boolean) => void;
  /** The query string of the range, for a link to another page. */
  search: (params?: Record<string, string | undefined>) => string;
}

export function useRange(): Range {
  const [params, setParams] = useSearchParams<{
    since?: string;
    until?: string;
    live?: string;
  }>();
  const since = () => params.since || DEFAULT_SINCE;
  const until = () => params.until ?? "";
  return {
    since,
    until,
    live: () => params.live === "1" && until() === "",
    setRange: (s, u) =>
      setParams({ since: s === DEFAULT_SINCE ? undefined : s, until: u || undefined }),
    setLive: (enabled) => setParams({ live: enabled ? "1" : undefined }),
    search: (more = {}) =>
      search({
        ...more,
        since: since() === DEFAULT_SINCE ? undefined : since(),
        until: until() || undefined,
      }),
  };
}

/** Fetches with `fetcher` for the range and `key`, again whenever either
 * changes, and every few seconds in live mode. */
export function createRangeFetch<K, T>(
  range: Range,
  key: Accessor<K>,
  fetcher: (key: K & { since: string; until: string }, signal: AbortSignal) => Promise<T>,
): Fetched<T> {
  const full = createMemo(
    () => ({ ...key(), since: range.since(), until: range.until() }),
    undefined,
    { equals: (a, b) => JSON.stringify(a) === JSON.stringify(b) },
  );
  const fetched = createFetch(full, fetcher);
  createEffect(() => {
    if (!range.live()) return;
    const timer = setInterval(() => {
      if (!fetched.loading()) fetched.reload();
    }, LIVE_MS);
    onCleanup(() => clearInterval(timer));
  });
  return fetched;
}
