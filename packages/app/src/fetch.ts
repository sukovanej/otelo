import { type Accessor, createEffect, createSignal, on, onCleanup } from "solid-js";

import { isAbortError } from "@otelo/api";

export interface FetchState<T> {
  readonly data: Accessor<T | undefined>;
  readonly errorMessage: Accessor<string | undefined>;
  readonly loading: Accessor<boolean>;
  readonly updatedAt: Accessor<Date | undefined>;
  readonly reload: () => void;
}

export function createFetch<K, T>(
  key: Accessor<K>,
  fetcher: (key: K, signal: AbortSignal) => Promise<T>,
): FetchState<T> {
  const [data, setData] = createSignal<T>();
  const [errorMessage, setErrorMessage] = createSignal<string>();
  const [loading, setLoading] = createSignal(false);
  const [updatedAt, setUpdatedAt] = createSignal<Date>();
  let controller: AbortController | undefined;

  const runRequest = (requestKey: K) => {
    controller?.abort();
    const current = new AbortController();
    controller = current;
    // The last result stays while the next request runs, so the page does
    // not blank on every change.
    setLoading(true);
    fetcher(requestKey, current.signal).then(
      (result) => {
        if (controller !== current) return;
        setData(() => result);
        setErrorMessage(undefined);
        setUpdatedAt(new Date());
        setLoading(false);
      },
      (error: unknown) => {
        if (controller !== current || isAbortError(error)) return;
        setErrorMessage(error instanceof Error ? error.message : String(error));
        setLoading(false);
      },
    );
  };

  createEffect(on(key, runRequest));
  onCleanup(() => controller?.abort());
  return { data, errorMessage, loading, updatedAt, reload: () => runRequest(key()) };
}
