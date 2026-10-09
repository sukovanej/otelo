import { createSignal, For, Show } from "solid-js";

import type { GroupedQuery } from "@otelo/api";
import { CloseIcon, PlusIcon } from "@otelo/icons";

import type { RangeState } from "../../services/range";
import { describeWidgetQuery, MAX_QUERIES_PER_TIMESERIES } from "../widget";
import WidgetEditorQuery from "./widget-editor-query";

const LETTER_A_CODE_POINT = 0x41;

interface WidgetEditorQueriesProps {
  readonly queries: ReadonlyArray<GroupedQuery>;
  readonly range: Pick<RangeState, "since" | "until" | "live">;
  readonly onChange: (queries: ReadonlyArray<GroupedQuery>) => void;
}

export default function WidgetEditorQueries(props: WidgetEditorQueriesProps) {
  const [chosenIndex, setChosenIndex] = createSignal(0);
  const shownIndex = () => Math.min(chosenIndex(), props.queries.length - 1);
  const shownQuery = () => props.queries[shownIndex()];
  const addQuery = () => {
    const lastQuery = props.queries.at(-1);
    if (!lastQuery) return;
    setChosenIndex(props.queries.length);
    props.onChange([...props.queries, lastQuery]);
  };
  const removeQuery = (index: number) => {
    setChosenIndex(index < shownIndex() ? shownIndex() - 1 : shownIndex());
    props.onChange(props.queries.toSpliced(index, 1));
  };
  const changeShownQuery = (changed: GroupedQuery) =>
    props.onChange(props.queries.with(shownIndex(), changed));

  return (
    <section class="flex flex-col rounded-lg border border-line" aria-label="Queries">
      <div class="flex flex-wrap items-end gap-x-1 border-b border-line px-2">
        <div class="flex flex-wrap" role="tablist" aria-label="Queries">
          <For each={props.queries} keyed={false}>
            {(grouped, index) => (
              <div
                class={[
                  "group/query -mb-px flex h-8 items-center border-b-2",
                  index === shownIndex()
                    ? "border-accent text-ink"
                    : "border-transparent text-muted hover:text-ink",
                ]}
              >
                <button
                  type="button"
                  role="tab"
                  aria-selected={index === shownIndex() ? "true" : "false"}
                  title={describeWidgetQuery(grouped().query)}
                  class="h-full cursor-pointer px-2 font-semibold"
                  onClick={() => setChosenIndex(index)}
                >
                  Query {nameQuery(index)}
                </button>
                <Show when={props.queries.length > 1}>
                  <button
                    type="button"
                    aria-label={`Remove query ${nameQuery(index)}`}
                    title={`Remove query ${nameQuery(index)}`}
                    class="mr-1 -ml-1 flex size-4 cursor-pointer items-center justify-center rounded-sm text-muted opacity-0 group-hover/query:opacity-100 hover:bg-hover hover:text-ink focus-visible:opacity-100 pointer-coarse:opacity-100"
                    onClick={() => removeQuery(index)}
                  >
                    <CloseIcon size={10} />
                  </button>
                </Show>
              </div>
            )}
          </For>
        </div>
        <Show when={props.queries.length < MAX_QUERIES_PER_TIMESERIES}>
          <button
            type="button"
            aria-label="Add a query"
            title="Add a query"
            class="mb-1 flex size-6 cursor-pointer items-center justify-center rounded text-muted hover:bg-hover hover:text-ink"
            onClick={addQuery}
          >
            <PlusIcon size={13} />
          </button>
        </Show>
      </div>
      <div class="p-3" role="tabpanel" aria-label={`Query ${nameQuery(shownIndex())}`}>
        <Show when={shownQuery()}>
          {(grouped) => (
            <WidgetEditorQuery
              grouped={grouped()}
              groupable
              range={props.range}
              onChange={changeShownQuery}
            />
          )}
        </Show>
      </div>
    </section>
  );
}

function nameQuery(index: number): string {
  return String.fromCodePoint(LETTER_A_CODE_POINT + index);
}
