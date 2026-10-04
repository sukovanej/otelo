import type { JSX } from "@solidjs/web";
import { createSignal, Show } from "solid-js";

import type { IndexedSignal } from "@otelo/api";
import { Callout } from "@otelo/ui";
import { Panel } from "@otelo/viz";

import { pageContent } from "../classes";
import FetchErrorBoundary from "../FetchErrorBoundary";
import type { ListResult, ListState } from "../list";
import ListContentEnd from "./list-content-end";
import ListContentIndexHint from "./list-content-index-hint";

interface IndexedListResult<V extends string> extends ListResult<V> {
  readonly body: IndexedListBody;
}

interface IndexedListBody {
  readonly truncated: boolean;
  readonly unindexed: ReadonlyArray<string>;
}

interface ListContentProps<V extends string, R extends IndexedListResult<V>, S extends string> {
  readonly list: ListState<V, R, S>;
  readonly signal: IndexedSignal;
  readonly singularNoun: string;
  readonly panel: JSX.Element;
  readonly children: JSX.Element;
}

export default function ListContent<
  V extends string,
  R extends IndexedListResult<V>,
  S extends string,
>(props: ListContentProps<V, R, S>) {
  const list = () => props.list;
  const [scrollElement, setScrollElement] = createSignal<HTMLDivElement>();
  return (
    <div class="flex min-h-0 flex-1">
      <div ref={setScrollElement} class={`min-w-0 flex-1 ${pageContent}`}>
        <Show when={list().fetched.errorMessage()}>
          {(errorMessage) => (
            <div class="mb-3">
              <Callout tone="error">{errorMessage()}</Callout>
            </div>
          )}
        </Show>

        <FetchErrorBoundary>
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

          <Show when={scrollElement()}>
            {(element) => (
              <ListContentEnd
                scrollElement={element()}
                canShowMore={list().canShowMore()}
                loading={list().fetched.loading()}
                onShowMore={() => list().showMore()}
              />
            )}
          </Show>
        </FetchErrorBoundary>
      </div>
      {props.panel}
    </div>
  );
}
