import { createSignal, For, Show } from "solid-js";

import { addIndex, type IndexedSignal } from "@otelo/api";
import { Button, Callout } from "@otelo/ui";

interface ListContentIndexHintProps {
  readonly signal: IndexedSignal;
  readonly singularNoun: string;
  readonly unindexedKeys: ReadonlyArray<string>;
}

export default function ListContentIndexHint(props: ListContentIndexHintProps) {
  const [indexedKeys, setIndexedKeys] = createSignal<ReadonlySet<string>>(new Set());
  const [errorMessage, setErrorMessage] = createSignal<string>();
  const indexAttribute = (key: string) =>
    addIndex(props.signal, key).then(
      () => setIndexedKeys((keys) => new Set([...keys, key])),
      (error: unknown) => setErrorMessage(error instanceof Error ? error.message : String(error)),
    );
  return (
    <Callout tone="hint">
      The query read every {props.singularNoun} in the range, because it compares attributes without
      an index.
      <For each={props.unindexedKeys}>
        {(key) => (
          <Show
            when={!indexedKeys().has(key)}
            fallback={<span>{key} is indexed; the writer builds it within seconds.</span>}
          >
            <Button size="sm" class="font-mono" onClick={() => void indexAttribute(key)}>
              Index {key}
            </Button>
          </Show>
        )}
      </For>
      <Show when={errorMessage()}>{(message) => <span class="text-error">{message()}</span>}</Show>
    </Callout>
  );
}
