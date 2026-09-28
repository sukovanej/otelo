import { Button, Callout, Checkbox, QueryInput, RangePicker, Tabs } from "@siner/ui";
import { useSearchParams } from "@solidjs/router";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  Match,
  on,
  onCleanup,
  Show,
  Switch,
} from "solid-js";
import {
  addIndex,
  complete,
  getLogGroups,
  getLogs,
  type ListParams,
  type LogGroups,
  type Logs,
} from "../api";
import { createFetch } from "../fetch";
import { addTerm } from "../query";
import { formatTime } from "../time";
import LogGroupList from "./LogGroupList";
import LogLines from "./LogLines";

type View = "lines" | "groups";

const VIEWS = [
  { value: "lines", label: "Lines" },
  { value: "groups", label: "Templates" },
] as const;

/** The first limit of a view, and how much "Show more" adds to it. */
const PAGE: Record<View, number> = { lines: 200, groups: 50 };

/** The most rows the API returns. */
const MAX_LIMIT = 10_000;

/** How often a live page reloads. */
const LIVE_MS = 5_000;

const DEFAULT_SINCE = "1h";

type Result = { view: "lines"; body: Logs } | { view: "groups"; body: LogGroups };

/**
 * Log lines or their message templates for a query and a range. The query,
 * the range, the view, and live mode live in the URL, so a link opens the
 * same page.
 */
export default function LogsPage() {
  const [params, setParams] = useSearchParams<{
    q?: string;
    since?: string;
    until?: string;
    view?: string;
    live?: string;
  }>();
  const q = () => params.q ?? "";
  const since = () => params.since || DEFAULT_SINCE;
  const until = () => params.until ?? "";
  const view = (): View => (params.view === "groups" ? "groups" : "lines");
  const live = () => params.live === "1" && until() === "";

  // The query as typed; it runs on Enter.
  const [draft, setDraft] = createSignal(q());
  createEffect(on(q, setDraft, { defer: true }));

  // "Show more" raises the limit of one query; any other query starts over.
  const base = () => JSON.stringify([view(), q(), since(), until()]);
  const [more, setMore] = createSignal({ base: "", limit: 0 });
  const limit = () => (more().base === base() ? more().limit : PAGE[view()]);

  const key = createMemo(
    () => ({ view: view(), q: q(), since: since(), until: until(), limit: limit() }),
    undefined,
    { equals: (a, b) => JSON.stringify(a) === JSON.stringify(b) },
  );
  const fetched = createFetch(
    key,
    async (k: ListParams & { view: View }, signal): Promise<Result> =>
      k.view === "groups"
        ? { view: "groups", body: await getLogGroups(k, signal) }
        : { view: "lines", body: await getLogs(k, signal) },
  );

  createEffect(() => {
    if (!live()) return;
    const timer = setInterval(() => {
      if (!fetched.loading()) fetched.reload();
    }, LIVE_MS);
    onCleanup(() => clearInterval(timer));
  });

  let queryInput: HTMLInputElement | undefined;
  const onSlash = (e: KeyboardEvent) => {
    const typing =
      e.target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName);
    if (e.key === "/" && !typing) {
      e.preventDefault();
      queryInput?.focus();
    }
  };
  document.addEventListener("keydown", onSlash);
  onCleanup(() => document.removeEventListener("keydown", onSlash));

  const run = () => {
    if (draft() === q()) fetched.reload();
    else setParams({ q: draft() || undefined });
  };
  const filter = (term: string) => setParams({ q: addTerm(q(), term) });

  const lines = () => {
    const result = fetched.data();
    return result?.view === "lines" && view() === "lines" ? result.body : undefined;
  };
  const groups = () => {
    const result = fetched.data();
    return result?.view === "groups" && view() === "groups" ? result.body : undefined;
  };
  const unindexed = () => fetched.data()?.body.unindexed ?? [];
  const truncated = () => fetched.data()?.body.truncated ?? false;

  return (
    <div class="px-4 pb-8">
      {/* The query and the view stay under the top bar while the results scroll. */}
      <div class="sticky top-11 z-20 -mx-4 border-b border-line bg-surface px-4 pt-3.5">
        <form
          class="flex items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            run();
          }}
        >
          <QueryInput
            value={draft()}
            complete={(text, cursor, signal) => complete("logs", text, cursor, signal)}
            onInput={setDraft}
            onSubmit={run}
            placeholder='level >= warn service = "api" body ~ "payment failed"'
            size="lg"
            ref={(el) => (queryInput = el)}
          />
          <RangePicker
            since={since()}
            until={until()}
            onChange={(s, u) =>
              setParams({ since: s === DEFAULT_SINCE ? undefined : s, until: u || undefined })
            }
            size="lg"
          />
          <Button type="submit" variant="primary" size="lg">
            Run
          </Button>
        </form>

        <div class="flex items-center gap-3 pt-3 pb-2.5 text-muted">
          <Tabs
            label="View"
            options={VIEWS}
            value={view()}
            onChange={(v) => setParams({ view: v === "lines" ? undefined : v })}
          />
          <span>
            <Switch>
              <Match when={lines()}>
                {(body) => (
                  <>
                    {count(body().logs.length, "line")}
                    {body().truncated ? ", newest first; more match" : ""}
                  </>
                )}
              </Match>
              <Match when={groups()}>
                {(body) => (
                  <>
                    {count(body().groups.length, "template")} of {count(body().scanned, "line")}
                    {body().partial ? " (the newest only)" : ""}
                  </>
                )}
              </Match>
            </Switch>
          </span>
          <span class="flex-1" />
          <Show when={fetched.loading()}>
            <span aria-live="polite">Loading…</span>
          </Show>
          <Show when={fetched.updated()}>
            {(updated) => <span>Updated {formatTime(updated())}</span>}
          </Show>
          <Checkbox
            checked={live()}
            disabled={until() !== ""}
            onChange={(checked) => setParams({ live: checked ? "1" : undefined })}
            title={
              until() === ""
                ? `Reload every ${LIVE_MS / 1000} s`
                : "Live needs a range that ends now"
            }
          >
            Live
          </Checkbox>
        </div>
      </div>

      <Show when={fetched.error()}>{(error) => <Callout tone="error">{error()}</Callout>}</Show>

      <Show when={unindexed().length > 0}>
        <IndexHint keys={unindexed()} />
      </Show>

      <Switch>
        <Match when={lines()}>
          {(body) => (
            <Show when={body().logs.length > 0} fallback={<Empty />}>
              <LogLines lines={body().logs} onFilter={filter} />
            </Show>
          )}
        </Match>
        <Match when={groups()}>
          {(body) => (
            <Show when={body().groups.length > 0} fallback={<Empty />}>
              <LogGroupList
                groups={body().groups}
                onShowLines={(term) => setParams({ q: addTerm(q(), term), view: undefined })}
              />
            </Show>
          )}
        </Match>
      </Switch>

      <Show when={truncated() && limit() < MAX_LIMIT}>
        <Button
          class="mx-auto mt-3 block"
          disabled={fetched.loading()}
          onClick={() => setMore({ base: base(), limit: Math.min(MAX_LIMIT, limit() * 2) })}
        >
          Show more
        </Button>
      </Show>
    </div>
  );
}

const count = (n: number, noun: string) => `${n.toLocaleString()} ${noun}${n === 1 ? "" : "s"}`;

function Empty() {
  return <div class="py-8 text-center text-muted">No lines in this range match the query.</div>;
}

/** Says which attributes made the query read every line, and indexes them. */
function IndexHint(props: { keys: string[] }) {
  const [done, setDone] = createSignal<ReadonlySet<string>>(new Set());
  const [error, setError] = createSignal<string>();
  const index = (key: string) =>
    addIndex("logs", key).then(
      () => setDone((keys) => new Set([...keys, key])),
      (e: unknown) => setError(e instanceof Error ? e.message : String(e)),
    );
  return (
    <Callout tone="hint">
      The query read every line in the range, because it compares attributes without an index.
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
