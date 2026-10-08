import { createSignal, For, Show, useContext } from "solid-js";

import type { IndexedSignal } from "@otelo/api";
import { Button, Callout } from "@otelo/ui";

import { ApiContext } from "../api";

type KeyIndexState = "indexing" | "indexed";

interface ListContentIndexHintProps {
  readonly signal: IndexedSignal;
  readonly singularNoun: string;
  readonly unindexedKeys: ReadonlyArray<string>;
}

export default function ListContentIndexHint(props: ListContentIndexHintProps) {
  const api = useContext(ApiContext);
  const [keyStates, setKeyStates] = createSignal<ReadonlyMap<string, KeyIndexState>>(new Map());
  const [errorMessage, setErrorMessage] = createSignal<string>();
  const setKeyState = (key: string, state: KeyIndexState | undefined) =>
    setKeyStates((states) => {
      const next = new Map(states);
      if (state === undefined) next.delete(key);
      else next.set(key, state);
      return next;
    });
  const indexAttribute = (key: string) => {
    setErrorMessage(undefined);
    setKeyState(key, "indexing");
    return api.addIndex(props.signal, key).then(
      () => setKeyState(key, "indexed"),
      (error: unknown) => {
        setKeyState(key, undefined);
        setErrorMessage(error instanceof Error ? error.message : String(error));
      },
    );
  };
  return (
    <Callout tone="hint">
      The query read every {props.singularNoun} in the range, because it compares attributes without
      an index.
      <For each={props.unindexedKeys}>
        {(key) => (
          <Show
            when={keyStates().get(key) !== "indexed"}
            fallback={<span>{key} is indexed; the writer builds it within seconds.</span>}
          >
            <Button
              size="sm"
              class="font-mono"
              disabled={keyStates().get(key) === "indexing"}
              onClick={() => void indexAttribute(key)}
            >
              Index {key}
            </Button>
          </Show>
        )}
      </For>
      <Show when={errorMessage()}>{(message) => <span class="text-error">{message()}</span>}</Show>
    </Callout>
  );
}
