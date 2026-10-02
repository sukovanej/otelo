import { useQuery } from "@tanstack/solid-query";
import { type Accessor, createMemo } from "solid-js";

export const LIVE_RELOAD_MS = 5_000;

export interface FetchState<T> {
  readonly data: Accessor<T | undefined>;
  readonly errorMessage: Accessor<string | undefined>;
  readonly loading: Accessor<boolean>;
  readonly updatedAt: Accessor<Date | undefined>;
  readonly reload: () => void;
}

export function createFetch<K, T>(
  queryName: string,
  key: Accessor<K>,
  fetcher: (key: K, signal: AbortSignal) => Promise<T>,
  live: Accessor<boolean> = () => false,
): FetchState<T> {
  const query = useQuery(() => {
    const requestKey = key();
    return {
      queryKey: [queryName, requestKey],
      queryFn: ({ signal }) => fetcher(requestKey, signal),
      refetchInterval: live() ? LIVE_RELOAD_MS : false,
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
