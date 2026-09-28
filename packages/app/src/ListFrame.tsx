import { Button, Callout, Checkbox, QueryInput, RangePicker, Tabs } from "@siner/ui";
import { createSignal, For, type JSX, Show } from "solid-js";
import { addIndex, complete, type Signal } from "@siner/api";
import { LIVE_MS, type List, type ListResult } from "./list";
import { formatTime } from "./time";

/**
 * The top of a list page, which stays under the top bar while the results
 * scroll: the query, the range, and Run, then the views, what the answer
 * holds (`children`), and live mode.
 */
export function QueryBar<V extends string, R extends ListResult<V>>(props: {
  list: List<V, R>;
  signal: Signal;
  placeholder: string;
  views: readonly { value: V; label: string }[];
  ref: (input: HTMLInputElement) => void;
  children: JSX.Element;
}) {
  const list = () => props.list;
  return (
    <div class="relative z-20 shrink-0 bg-surface px-4 pt-3.5 shadow-(--raised)">
      <form
        class="flex items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          list().run();
        }}
      >
        <QueryInput
          value={list().draft()}
          complete={(text, cursor, signal) => complete(props.signal, text, cursor, signal)}
          onInput={(q) => list().setDraft(q)}
          onSubmit={() => list().run()}
          placeholder={props.placeholder}
          size="lg"
          ref={props.ref}
        />
        <RangePicker
          since={list().since()}
          until={list().until()}
          onChange={(since, until) => list().setRange(since, until)}
          size="lg"
        />
        <Button type="submit" variant="primary" size="lg">
          Run
        </Button>
      </form>

      <div class="flex items-center gap-3 pt-3 pb-2.5 text-muted">
        <Tabs
          label="View"
          options={props.views}
          value={list().view()}
          onChange={(v) => list().setView(v)}
        />
        <span>{props.children}</span>
        <span class="flex-1" />
        <Show when={list().fetched.loading()}>
          <span aria-live="polite">Loading…</span>
        </Show>
        <Show when={list().fetched.updated()}>
          {(updated) => <span>Updated {formatTime(updated())}</span>}
        </Show>
        <Checkbox
          checked={list().live()}
          disabled={list().until() !== ""}
          onChange={(checked) => list().setLive(checked)}
          title={
            list().until() === ""
              ? `Reload every ${LIVE_MS / 1000} s`
              : "Live needs a range that ends now"
          }
        >
          Live
        </Checkbox>
      </div>
    </div>
  );
}

/**
 * The results of a list page, which scroll under the query bar: an error, a
 * hint to index the attributes the query compared, the rows (`children`),
 * and "Show more", with the `panel` of a selected row beside them.
 */
export function ListContent<V extends string, R extends ListResult<V>>(props: {
  list: List<V, R>;
  signal: Signal;
  /** What the list holds, such as `line`, for the index hint. */
  noun: string;
  panel?: JSX.Element;
  children: JSX.Element;
}) {
  const list = () => props.list;
  return (
    <div class="flex min-h-0 flex-1">
      <div class="min-w-0 flex-1 overflow-y-auto px-4 pb-8">
        <Show when={list().fetched.error()}>
          {(error) => (
            <div class="mt-2">
              <Callout tone="error">{error()}</Callout>
            </div>
          )}
        </Show>

        <Show when={list().current()?.body.unindexed.length}>
          <div class="mt-2">
            <IndexHint
              signal={props.signal}
              noun={props.noun}
              keys={list().current()?.body.unindexed ?? []}
            />
          </div>
        </Show>

        {props.children}

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

/** The line under a list that has no rows. */
export function Empty(props: { children: JSX.Element }) {
  return <div class="py-8 text-center text-muted">{props.children}</div>;
}

/** Says which attributes made the query read every record, and indexes them. */
function IndexHint(props: { signal: Signal; noun: string; keys: string[] }) {
  const [done, setDone] = createSignal<ReadonlySet<string>>(new Set());
  const [error, setError] = createSignal<string>();
  const index = (key: string) =>
    addIndex(props.signal, key).then(
      () => setDone((keys) => new Set([...keys, key])),
      (e: unknown) => setError(e instanceof Error ? e.message : String(e)),
    );
  return (
    <Callout tone="hint">
      The query read every {props.noun} in the range, because it compares attributes without an
      index.
      <For each={props.keys}>
        {(key) => (
          <Show
            when={!done().has(key)}
            fallback={<span>{key} is indexed; the writer builds it within seconds.</span>}
          >
            <Button size="sm" class="font-mono" onClick={() => index(key)}>
              Index {key}
            </Button>
          </Show>
        )}
      </For>
      <Show when={error()}>{(message) => <span class="text-error">{message()}</span>}</Show>
    </Callout>
  );
}
