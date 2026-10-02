import { type SearchParams, useSearchParams } from "@solidjs/router";
import { useQuery } from "@tanstack/solid-query";
import { type Accessor, createMemo } from "solid-js";

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

export function createRangeFetch<K, T>(
  range: LiveRange,
  queryName: string,
  key: Accessor<K>,
  fetcher: (key: K & RangeBounds, signal: AbortSignal) => Promise<T>,
): FetchState<T> {
  const query = useQuery(() => {
    const keyWithRange = { ...key(), since: range.since(), until: range.until() };
    return {
      queryKey: [queryName, keyWithRange],
      queryFn: ({ signal }) => fetcher(keyWithRange, signal),
      refetchInterval: range.live() ? LIVE_RELOAD_MS : false,
    };
  });
  const answered = createMemo((wasAnswered) => wasAnswered === true || query.isSuccess);
  const updatedAt = createMemo<Date | undefined>((previous) =>
    query.dataUpdatedAt === 0 ? previous : new Date(query.dataUpdatedAt),
  );
  return {
    data: () => (answered() ? query.data : undefined),
    errorMessage: () => (!answered() || query.isRefetchError ? query.error?.message : undefined),
    loading: () => query.isFetching,
    updatedAt,
    reload: () => void query.refetch(),
  };
}
