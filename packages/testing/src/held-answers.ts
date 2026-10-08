import { createDeferred, type Deferred } from "./deferred";

export interface HeldAnswers<T> {
  readonly answer: (signal?: AbortSignal) => Promise<T>;
  readonly held: ReadonlyArray<Deferred<T>>;
  readonly releaseAll: (value: T) => void;
}

// The first request answers at once, so the page shows something; every later one waits until
// the test releases it.
export function createHeldAnswers<T>(firstAnswer: T): HeldAnswers<T> {
  const held: Deferred<T>[] = [];
  let hasAnswered = false;
  return {
    answer: (signal) => {
      if (!hasAnswered) {
        hasAnswered = true;
        return Promise.resolve(firstAnswer);
      }
      const answer = createDeferred<T>(signal);
      held.push(answer);
      return answer.promise;
    },
    held,
    releaseAll: (value) => {
      for (const answer of held) answer.resolve(value);
    },
  };
}
