import { type SearchParams, useSearchParams } from "@solidjs/router";
import { keepPreviousData, QueryObserver } from "@tanstack/query-core";
import {
  type Accessor,
  createEffect,
  createMemo,
  createSignal,
  onSettled,
  untrack,
} from "solid-js";

import { toQueryString } from "@otelo/api";

import type { FetchState } from "../fetch";
import { DEFAULT_SINCE, LIVE_RELOAD_MS } from "../list";
import { queryClient } from "../queryClient";

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

export function createRangeFetch<K, T>(
  range: LiveRange,
  queryName: string,
  key: Accessor<K>,
  fetcher: (key: K & RangeBounds, signal: AbortSignal) => Promise<T>,
): FetchState<T> {
  const options = createMemo(() => {
    const keyWithRange = { ...key(), since: range.since(), until: range.until() };
    return {
      queryKey: [queryName, keyWithRange],
      queryFn: ({ signal }: { signal: AbortSignal }) => fetcher(keyWithRange, signal),
      refetchInterval: range.live() ? LIVE_RELOAD_MS : false,
      placeholderData: keepPreviousData,
    } as const;
  });
  const observer = new QueryObserver(queryClient, untrack(options));
  const [result, setResult] = createSignal(observer.getCurrentResult());
  onSettled(() => observer.subscribe(setResult));
  createEffect(options, (newOptions) => observer.setOptions(newOptions));
  const updatedAt = createMemo<Date | undefined>((previous) => {
    const fetchedAt = result().dataUpdatedAt;
    if (fetchedAt === 0 || fetchedAt === previous?.getTime()) return previous;
    return new Date(fetchedAt);
  });
  return {
    data: () => result().data,
    errorMessage: () => result().error?.message,
    loading: () => result().isFetching,
    updatedAt,
    reload: () => void observer.refetch(),
  };
}
