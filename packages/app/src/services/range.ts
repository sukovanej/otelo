import { type SearchParams, useSearchParams } from "@solidjs/router";
import {
  type Accessor,
  action,
  affects,
  createEffect,
  createMemo,
  isPending,
  onCleanup,
  refresh,
} from "solid-js";

import { toQueryString } from "@otelo/api";

import type { FetchState } from "../fetch";
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
      toQueryString({
        ...extraParams,
        since: since() === DEFAULT_SINCE ? undefined : since(),
        until: until() || undefined,
      }),
  };
}

type LiveRange = Pick<RangeState, "since" | "until" | "live">;

interface RangeBounds {
  readonly since: string;
  readonly until: string;
}

type RangeAnswer<T> = FetchedAnswer<T> | FailedAnswer<T>;

interface FetchedAnswer<T> {
  readonly state: "fetched";
  readonly data: T;
  readonly updatedAt: Date;
}

interface FailedAnswer<T> {
  readonly state: "failed";
  readonly errorMessage: string;
  readonly lastFetched: FetchedAnswer<T> | undefined;
}

export function createRangeFetch<K, T>(
  range: LiveRange,
  key: Accessor<K>,
  fetcher: (key: K & RangeBounds, signal: AbortSignal) => Promise<T>,
): FetchState<T> {
  const keyWithRange = createMemo(
    () => ({ ...key(), since: range.since(), until: range.until() }),
    { equals: (previous, next) => JSON.stringify(previous) === JSON.stringify(next) },
  );
  const answer = createMemo<RangeAnswer<T> | undefined>(
    async (previous) => {
      const controller = new AbortController();
      onCleanup(() => controller.abort());
      try {
        const data = await fetcher(keyWithRange(), controller.signal);
        return { state: "fetched", data, updatedAt: new Date() };
      } catch (error) {
        return {
          state: "failed",
          errorMessage: error instanceof Error ? error.message : String(error),
          lastFetched: previous?.state === "fetched" ? previous : previous?.lastFetched,
        };
      }
    },
    { loadingValue: undefined },
  );
  const lastFetched = () => {
    const shownAnswer = answer();
    return shownAnswer?.state === "failed" ? shownAnswer.lastFetched : shownAnswer;
  };
  const loading = () => answer() === undefined || isPending(answer);
  const reload = action(function* () {
    affects(answer);
    yield refresh(answer);
  });
  createEffect(range.live, (isLive) => {
    if (!isLive) return undefined;
    const timer = setInterval(() => {
      if (!loading()) void reload();
    }, LIVE_RELOAD_MS);
    return () => clearInterval(timer);
  });
  return {
    data: () => lastFetched()?.data,
    errorMessage: () => {
      const shownAnswer = answer();
      return shownAnswer?.state === "failed" ? shownAnswer.errorMessage : undefined;
    },
    loading,
    updatedAt: () => lastFetched()?.updatedAt,
    reload: () => void reload(),
  };
}
