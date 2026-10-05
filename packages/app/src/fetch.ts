import {
  type InfiniteData,
  type QueryClient,
  replaceEqualDeep,
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/solid-query";
import { type Accessor, createMemo, isPending } from "solid-js";

export const LIVE_RELOAD_MS = 5_000;

export interface FetchState<T> {
  readonly data: Accessor<T | undefined>;
  readonly errorMessage: Accessor<string | undefined>;
  readonly loading: Accessor<boolean>;
  readonly updatedAt: Accessor<Date | undefined>;
  readonly reload: () => void;
}

export interface PagedFetchState<T> extends FetchState<T> {
  readonly hasNextPage: Accessor<boolean>;
  readonly pageCount: Accessor<number>;
  readonly fetchNextPage: () => void;
}

export interface PageJoining<T> {
  readonly readNextCursor: (page: T) => string | undefined;
  readonly joinPages: (shown: T, page: T) => T;
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

export function createPagedFetch<K, T>(
  queryName: string,
  key: Accessor<K>,
  fetcher: (key: K, after: string | undefined, signal: AbortSignal) => Promise<T>,
  pageJoining: PageJoining<T> | undefined,
  live: Accessor<boolean> = () => false,
): PagedFetchState<T> {
  const query = useInfiniteQuery(() => {
    const requestKey = key();
    const isLive = live();
    return {
      // A refresh reads every loaded page again, so a live list keeps its own single page.
      queryKey: [queryName, requestKey, isLive],
      queryFn: ({ pageParam, signal }) => fetcher(requestKey, pageParam, signal),
      initialPageParam: undefined as string | undefined,
      getNextPageParam: (page: T) => (isLive ? undefined : pageJoining?.readNextCursor(page)),
      refetchInterval: isLive ? LIVE_RELOAD_MS : false,
    };
  });
  const answered = createMemo((wasAnswered) => wasAnswered === true || query.isSuccess);
  const updatedAt = createMemo<Date | undefined>((previous) =>
    query.dataUpdatedAt === 0 ? previous : new Date(query.dataUpdatedAt),
  );
  // A new key leaves the pages undefined until its first page lands.
  const readPages = (): ReadonlyArray<T> => {
    const data: InfiniteData<T> | undefined = answered() ? query.data : undefined;
    return data?.pages ?? [];
  };
  const joinedPages = createMemo(() => {
    const [firstPage, ...nextPages] = readPages();
    return firstPage && pageJoining
      ? nextPages.reduce(pageJoining.joinPages, firstPage)
      : firstPage;
  });
  return {
    data: joinedPages,
    errorMessage: () => (!answered() || query.isRefetchError ? query.error?.message : undefined),
    loading: () => query.isFetching || isPending(() => query.data),
    updatedAt,
    reload: () => void query.refetch(),
    hasNextPage: () => query.hasNextPage,
    pageCount: () => readPages().length,
    fetchNextPage: () => void query.fetchNextPage(),
  };
}

// A refresh reads every page a list holds, so a list nobody shows keeps its first page only,
// and showing it again reads one page.
export function trimUnwatchedListsToFirstPage(queryClient: QueryClient): void {
  queryClient.getQueryCache().subscribe((event) => {
    if (event.type !== "observerRemoved") return;
    const { query } = event;
    // The observer leaves while Solid disposes the page, so the write waits until it is done.
    setTimeout(() => {
      const data: unknown = query.state.data;
      if (query.getObserversCount() > 0 || !isInfiniteData(data) || data.pages.length < 2) return;
      queryClient.setQueryData(
        query.queryKey,
        { pages: data.pages.slice(0, 1), pageParams: data.pageParams.slice(0, 1) },
        { updatedAt: query.state.dataUpdatedAt },
      );
    }, 0);
  });
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

function isInfiniteData(data: unknown): data is InfiniteData<unknown> {
  return (
    typeof data === "object" &&
    data !== null &&
    "pages" in data &&
    Array.isArray(data.pages) &&
    "pageParams" in data &&
    Array.isArray(data.pageParams)
  );
}
