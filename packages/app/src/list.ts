import { type SearchParams, useSearchParams } from "@solidjs/router";
import { createSignal, onCleanup } from "solid-js";

import type { ListQuery } from "@otelo/api";

import { createFetch, type FetchState } from "./fetch";
import { addTerm } from "./query";

export const DEFAULT_SINCE = "1h";

export const API_MAX_ROWS = 10_000;

export interface ListResult<V extends string> {
  readonly view: V;
  readonly body: ListBody;
}

export interface ListState<V extends string, R extends ListResult<V>> {
  readonly query: () => string;
  readonly since: () => string;
  readonly until: () => string;
  readonly view: () => V;
  readonly live: () => boolean;
  readonly draftQuery: () => string;
  readonly setDraftQuery: (query: string) => void;
  readonly runDraftQuery: () => void;
  readonly addTerm: (term: string, view?: V) => void;
  readonly setView: (view: V) => void;
  readonly setRange: (since: string, until: string) => void;
  readonly setLive: (live: boolean) => void;
  readonly fetched: FetchState<R>;
  readonly shownResult: () => R | undefined;
  readonly canShowMore: () => boolean;
  readonly showMore: () => void;
}

interface ListBody {
  readonly truncated: boolean;
}

interface ListKey<V extends string> extends ListQuery {
  readonly view: V;
}

type ListViews<V extends string> = readonly [defaultView: V, ...otherViews: V[]];

interface ListOptions<V extends string, R extends ListResult<V>> {
  readonly name: string;
  readonly views: ListViews<V>;
  readonly firstLimits: Record<V, number>;
  readonly fetch: (key: ListKey<V>, signal: AbortSignal) => Promise<R>;
}

interface ListSearchParams extends SearchParams {
  readonly q?: string;
  readonly since?: string;
  readonly until?: string;
  readonly view?: string;
  readonly live?: string;
}

export function createListState<V extends string, R extends ListResult<V>>(
  options: ListOptions<V, R>,
): ListState<V, R> {
  const [params, setParams] = useSearchParams<ListSearchParams>();
  const [defaultView] = options.views;
  const query = () => params.q ?? "";
  const since = () => params.since || DEFAULT_SINCE;
  const until = () => params.until ?? "";
  const view = () => options.views.find((knownView) => knownView === params.view) ?? defaultView;
  const live = () => params.live === "1" && until() === "";
  const toViewParam = (newView: V) => (newView === defaultView ? undefined : newView);

  const [draftQuery, setDraftQuery] = createSignal(() => query());

  const queryIdentity = () => JSON.stringify([view(), query(), since(), until()]);
  const [raisedLimit, setRaisedLimit] = createSignal({ forQuery: "", limit: 0 });
  const limit = () =>
    raisedLimit().forQuery === queryIdentity() ? raisedLimit().limit : options.firstLimits[view()];

  const fetched = createFetch(
    options.name,
    () => ({ view: view(), q: query().trim(), since: since(), until: until(), limit: limit() }),
    options.fetch,
    live,
  );

  const shownResult = () => {
    const result = fetched.data();
    return result?.view === view() ? result : undefined;
  };

  return {
    query,
    since,
    until,
    view,
    live,
    draftQuery,
    setDraftQuery,
    runDraftQuery: () => {
      if (draftQuery() === query()) fetched.reload();
      else setParams({ q: draftQuery() || undefined });
    },
    addTerm: (term, newView = view()) =>
      setParams({ q: addTerm(query(), term), view: toViewParam(newView) }),
    setView: (newView) => setParams({ view: toViewParam(newView) }),
    setRange: (newSince, newUntil) =>
      setParams({
        since: newSince === DEFAULT_SINCE ? undefined : newSince,
        until: newUntil || undefined,
      }),
    setLive: (enabled) => setParams({ live: enabled ? "1" : undefined }),
    fetched,
    shownResult,
    canShowMore: () => (shownResult()?.body.truncated ?? false) && limit() < API_MAX_ROWS,
    showMore: () =>
      setRaisedLimit({ forQuery: queryIdentity(), limit: Math.min(API_MAX_ROWS, limit() * 2) }),
  };
}

interface PageKeys {
  readonly queryInput?: () => HTMLInputElement | undefined;
  readonly onEscape?: () => void;
}

export function usePageKeys(keys: PageKeys): void {
  const onKeyDown = (e: KeyboardEvent) => {
    const isTyping =
      e.target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName);
    if (isTyping || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === "/" && keys.queryInput) {
      e.preventDefault();
      keys.queryInput()?.focus();
    } else if (e.key === "Escape" && keys.onEscape) {
      keys.onEscape();
    }
  };
  document.addEventListener("keydown", onKeyDown);
  onCleanup(() => document.removeEventListener("keydown", onKeyDown));
}
