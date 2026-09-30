import { type SearchParams, useSearchParams } from "@solidjs/router";
import { type Accessor, createEffect, createMemo, onCleanup } from "solid-js";

import { search } from "@otelo/api";

import { createFetch, type FetchState } from "../fetch";
import { DEFAULT_SINCE, LIVE_RELOAD_MS } from "../list";

export interface RangeState {
  readonly since: () => string;
  readonly until: () => string;
  readonly live: () => boolean;
  readonly setRange: (since: string, until: string) => void;
  readonly setLive: (live: boolean) => void;
  readonly toSearch: (params?: Record<string, string | undefined>) => string;
}

interface RangeSearchParams extends SearchParams {
  readonly since?: string;
  readonly until?: string;
  readonly live?: string;
}

export function useRange(): RangeState {
  const [params, setParams] = useSearchParams<RangeSearchParams>();
  const since = () => params.since || DEFAULT_SINCE;
  const until = () => params.until ?? "";
  return {
    since,
    until,
    live: () => params.live === "1" && until() === "",
    setRange: (newSince, newUntil) =>
      setParams({
        since: newSince === DEFAULT_SINCE ? undefined : newSince,
        until: newUntil || undefined,
      }),
    setLive: (enabled) => setParams({ live: enabled ? "1" : undefined }),
    toSearch: (extraParams = {}) =>
      search({
        ...extraParams,
        since: since() === DEFAULT_SINCE ? undefined : since(),
        until: until() || undefined,
      }),
  };
}

interface RangeBounds {
  readonly since: string;
  readonly until: string;
}

export function createRangeFetch<K, T>(
  range: RangeState,
  key: Accessor<K>,
  fetcher: (key: K & RangeBounds, signal: AbortSignal) => Promise<T>,
): FetchState<T> {
  const keyWithRange = createMemo(
    () => ({ ...key(), since: range.since(), until: range.until() }),
    undefined,
    { equals: (previous, next) => JSON.stringify(previous) === JSON.stringify(next) },
  );
  const fetched = createFetch(keyWithRange, fetcher);
  createEffect(() => {
    if (!range.live()) return;
    const timer = setInterval(() => {
      if (!fetched.loading()) fetched.reload();
    }, LIVE_RELOAD_MS);
    onCleanup(() => clearInterval(timer));
  });
  return fetched;
}
