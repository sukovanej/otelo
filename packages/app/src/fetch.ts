import { type Accessor, createEffect, createSignal, on, onCleanup } from "solid-js";
import { aborted } from "@siner/api";

export interface Fetched<T> {
  /** The last result that arrived. It stays while a new request runs, so the
   * page does not blank on every change. */
  data: Accessor<T | undefined>;
  /** The message of the last request's error, cleared by the next result. */
  error: Accessor<string | undefined>;
  loading: Accessor<boolean>;
  /** When the last result arrived. */
  updated: Accessor<Date | undefined>;
  /** Runs the request again with the same key. */
  reload: () => void;
}

/**
 * Runs `fetcher` whenever `key` changes, and aborts the request before it
 * when that one has not answered yet.
 */
export function createFetch<K, T>(
  key: Accessor<K>,
  fetcher: (key: K, signal: AbortSignal) => Promise<T>,
): Fetched<T> {
  const [data, setData] = createSignal<T>();
  const [error, setError] = createSignal<string>();
  const [loading, setLoading] = createSignal(false);
  const [updated, setUpdated] = createSignal<Date>();
  let controller: AbortController | undefined;

  const run = (k: K) => {
    controller?.abort();
    const current = new AbortController();
    controller = current;
    setLoading(true);
    fetcher(k, current.signal).then(
      (result) => {
        if (controller !== current) return;
        setData(() => result);
        setError(undefined);
        setUpdated(new Date());
        setLoading(false);
      },
      (e: unknown) => {
        if (controller !== current || aborted(e)) return;
        setError(e instanceof Error ? e.message : String(e));
        setLoading(false);
      },
    );
  };

  createEffect(on(key, run));
  onCleanup(() => controller?.abort());
  return { data, error, loading, updated, reload: () => run(key()) };
}
