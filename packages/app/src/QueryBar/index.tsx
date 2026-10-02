import type { JSX } from "@solidjs/web";
import { latest, Show } from "solid-js";

import { completeQuery, type Signal } from "@otelo/api";
import { Button, QueryInput, RangePicker, Tabs } from "@otelo/ui";

import { describeFieldOfToken } from "../fieldHelp";
import { highlightQuery } from "../highlight";
import type { ListResult, ListState } from "../list";
import LiveToggle from "../LiveToggle";
import PageBar from "../PageBar";
import QueryBarFieldCard from "./query-bar-field-card";

interface ViewOption<V extends string> {
  readonly value: V;
  readonly label: string;
}

interface QueryBarProps<V extends string, R extends ListResult<V>> {
  readonly list: ListState<V, R>;
  readonly signal: Signal;
  readonly placeholder: string;
  readonly views: ReadonlyArray<ViewOption<V>>;
  readonly ref: (input: HTMLInputElement) => void;
  readonly children: JSX.Element;
}

export default function QueryBar<V extends string, R extends ListResult<V>>(
  props: QueryBarProps<V, R>,
) {
  const list = () => props.list;
  return (
    <PageBar
      fetched={list().fetched}
      top={
        <form
          class="flex items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            list().runDraftQuery();
          }}
        >
          <QueryInput
            value={list().draftQuery()}
            highlight={highlightQuery}
            complete={(query, cursorInChars, abort) =>
              completeQuery(props.signal, query, cursorInChars, abort).then(
                (completions) => completions.suggestions,
              )
            }
            help={(query, token, abort) =>
              describeFieldOfToken(props.signal, query, token, abort).then(
                (field) =>
                  field && (() => <QueryBarFieldCard field={field} signal={props.signal} />),
              )
            }
            onInput={(query) => list().setDraftQuery(query)}
            onSubmit={() => list().runDraftQuery()}
            placeholder={props.placeholder}
            size="lg"
            ref={props.ref}
          />
          <RangePicker
            since={latest(list().since)}
            until={latest(list().until)}
            onChange={(since, until) => list().setRange(since, until)}
            size="lg"
          />
          <Button type="submit" variant="primary" size="lg">
            Run
          </Button>
        </form>
      }
      end={
        <LiveToggle
          live={latest(list().live)}
          until={latest(list().until)}
          onChange={(live) => list().setLive(live)}
        />
      }
    >
      <Show when={props.views.length > 1}>
        <Tabs
          label="View"
          options={props.views}
          value={list().view()}
          onChange={(view) => list().setView(view)}
        />
      </Show>
      <span>{props.children}</span>
    </PageBar>
  );
}
