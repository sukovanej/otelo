export interface Deferred<T> {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
  readonly reject: (reason: Error) => void;
}

// A promise the test settles when it chooses, so it can act while a request is in the air.
// Like fetch, it rejects when its request is aborted.
export function createDeferred<T>(abortSignal?: AbortSignal): Deferred<T> {
  let resolve: (value: T) => void = ignoreSettlement;
  let reject: (reason: Error) => void = ignoreSettlement;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  abortSignal?.addEventListener("abort", () => reject(new DOMException("Aborted", "AbortError")));
  return { promise, resolve, reject };
}

function ignoreSettlement(): void {}
