import type { JSX } from "@solidjs/web";
import { Show } from "solid-js";

import type { IndexedSignal } from "@otelo/api";
import { Button, Callout } from "@otelo/ui";
import { Panel } from "@otelo/viz";

import { pageContent } from "../classes";
import type { ListResult, ListState } from "../list";
import ListContentIndexHint from "./list-content-index-hint";

interface IndexedListResult<V extends string> extends ListResult<V> {
  readonly body: IndexedListBody;
}

interface IndexedListBody {
  readonly truncated: boolean;
  readonly unindexed: ReadonlyArray<string>;
}

interface ListContentProps<V extends string, R extends IndexedListResult<V>> {
  readonly list: ListState<V, R>;
  readonly signal: IndexedSignal;
  readonly singularNoun: string;
  readonly panel: JSX.Element;
  readonly children: JSX.Element;
}

export default function ListContent<V extends string, R extends IndexedListResult<V>>(
  props: ListContentProps<V, R>,
) {
  const list = () => props.list;
  return (
    <div class="flex min-h-0 flex-1">
      <div class={`min-w-0 flex-1 ${pageContent}`}>
        <Show when={list().fetched.errorMessage()}>
          {(errorMessage) => (
            <div class="mb-3">
              <Callout tone="error">{errorMessage()}</Callout>
            </div>
          )}
        </Show>

        <Show when={list().shownResult()?.body.unindexed.length}>
          <div class="mb-3">
            <ListContentIndexHint
              signal={props.signal}
              singularNoun={props.singularNoun}
              unindexedKeys={list().shownResult()?.body.unindexed ?? []}
            />
          </div>
        </Show>

        <Panel flush>{props.children}</Panel>

        <Show when={list().canShowMore()}>
          <Button
            class="mx-auto mt-3 block"
            disabled={list().fetched.loading()}
            onClick={() => list().showMore()}
          >
            Show more
          </Button>
        </Show>
      </div>
      {props.panel}
    </div>
  );
}
