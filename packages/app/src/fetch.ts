import { replaceEqualDeep, useQuery, useQueryClient } from "@tanstack/solid-query";
import { type Accessor, createMemo, isPending } from "solid-js";

export const LIVE_RELOAD_MS = 5_000;

export interface FetchState<T> {
  readonly data: Accessor<T | undefined>;
  readonly errorMessage: Accessor<string | undefined>;
  readonly loading: Accessor<boolean>;
  readonly updatedAt: Accessor<Date | undefined>;
  readonly reload: () => void;
}

interface ReplaceableFetchState<T> extends FetchState<T> {
  readonly replaceData: (data: T) => void;
}

export function createFetch<K, T>(
  queryName: string,
  key: Accessor<K>,
  fetcher: (key: K, signal: AbortSignal) => Promise<T>,
  live: Accessor<boolean> = () => false,
): ReplaceableFetchState<T> {
  const queryClient = useQueryClient();
  const query = useQuery(() => {
    const requestKey = key();
    return {
      queryKey: [queryName, requestKey],
      queryFn: ({ signal }) => fetcher(requestKey, signal),
      // A frozen answer goes to the store whole; sharing parts of the last one
      // would hand it new containers that are not frozen.
      structuralSharing: (oldData: unknown, newData: unknown) =>
        Object.isFrozen(newData) ? newData : replaceEqualDeep(oldData, newData),
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
    loading: () => query.isFetching || isPending(() => query.data),
    updatedAt,
    reload: () => void query.refetch(),
    replaceData: (data) => queryClient.setQueryData([queryName, key()], data),
  };
}

export function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

// A frozen answer stays out of the store of its query, so a reader of many of
// its values, such as a chart, subscribes to the answer once and not to each value.
export function freezeDeeply<T>(value: T): T {
  if (typeof value === "object" && value !== null && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) freezeDeeply(child);
    Object.freeze(value);
  }
  return value;
}
