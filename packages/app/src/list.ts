import { type SearchParams, useSearchParams } from "@solidjs/router";
import { createSignal, latest, onCleanup } from "solid-js";

import type { ListQuery } from "@otelo/api";

import { createPagedFetch, type PageJoining, type PagedFetchState } from "./fetch";
import { addTerm } from "./query";

export const DEFAULT_SINCE = "1h";

export const API_MAX_ROWS = 10_000;

export interface ListResult<V extends string> {
  readonly view: V;
  readonly body: ListBody;
}

export interface ListState<V extends string, R extends ListResult<V>, S extends string = never> {
  readonly query: () => string;
  readonly since: () => string;
  readonly until: () => string;
  readonly view: () => V;
  readonly sortOrder: () => S | undefined;
  readonly live: () => boolean;
  readonly draftQuery: () => string;
  readonly setDraftQuery: (query: string) => void;
  readonly runDraftQuery: () => void;
  readonly addTerm: (term: string, view?: V) => void;
  readonly setView: (view: V) => void;
  readonly setSortOrder: (sort: S) => void;
  readonly setRange: (since: string, until: string) => void;
  readonly setLive: (live: boolean) => void;
  readonly fetched: PagedFetchState<R>;
  readonly shownResult: () => R | undefined;
  readonly canShowMore: () => boolean;
  readonly showMore: () => void;
}

type ListBody = PagedListBody | TruncatedListBody;

interface PagedListBody {
  readonly next: string | null;
}

interface TruncatedListBody {
  readonly truncated: boolean;
}

interface ListKey<V extends string, S extends string> extends ListQuery {
  readonly view: V;
  readonly sort?: S;
}

type ListViews<V extends string> = readonly [defaultView: V, ...otherViews: V[]];

type ListSorts<S extends string> = readonly [defaultSort: S, ...otherSorts: S[]];

interface ListOptions<V extends string, R extends ListResult<V>, S extends string> {
  readonly name: string;
  readonly views: ListViews<V>;
  readonly sorts?: ListSorts<S>;
  readonly firstLimits: Record<V, number>;
  readonly fetch: (key: ListKey<V, S>, signal: AbortSignal) => Promise<R>;
  readonly pageJoining?: PageJoining<R>;
}

interface ListSearchParams extends SearchParams {
  readonly q?: string;
  readonly since?: string;
  readonly until?: string;
  readonly view?: string;
  readonly sort?: string;
  readonly live?: string;
}

export function createListState<
  V extends string,
  R extends ListResult<V>,
  S extends string = never,
>(options: ListOptions<V, R, S>): ListState<V, R, S> {
  const [params, setParams] = useSearchParams<ListSearchParams>();
  const [defaultView] = options.views;
  const query = () => params.q ?? "";
  const since = () => params.since || DEFAULT_SINCE;
  const until = () => params.until ?? "";
  const view = () => options.views.find((knownView) => knownView === params.view) ?? defaultView;
  const defaultSort = options.sorts?.[0];
  const sortOrder = () =>
    options.sorts?.find((knownSort) => knownSort === params.sort) ?? defaultSort;
  const live = () => params.live === "1" && until() === "";
  const toViewParam = (newView: V) => (newView === defaultView ? undefined : newView);

  const [draftQuery, setDraftQuery] = createSignal(() => query());

  const queryIdentity = () => JSON.stringify([view(), sortOrder(), query(), since(), until()]);
  const [raisedLimit, setRaisedLimit] = createSignal({ forQuery: "", limit: 0 });
  const limit = () =>
    raisedLimit().forQuery === queryIdentity() ? raisedLimit().limit : options.firstLimits[view()];

  const fetched = createPagedFetch(
    options.name,
    (): ListKey<V, S> => {
      const sort = sortOrder();
      return {
        view: view(),
        ...(sort !== undefined && { sort }),
        q: query().trim(),
        since: since(),
        until: until(),
        limit: limit(),
      };
    },
    (key, after, signal) => options.fetch(after === undefined ? key : { ...key, after }, signal),
    options.pageJoining,
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
    sortOrder,
    live,
    draftQuery,
    setDraftQuery,
    runDraftQuery: () => {
      if (draftQuery() === query()) fetched.reload();
      else setParams({ q: draftQuery() || undefined });
    },
    addTerm: (term, newView = latest(view)) =>
      setParams({ q: addTerm(latest(query), term), view: toViewParam(newView) }),
    setView: (newView) => setParams({ view: toViewParam(newView) }),
    setSortOrder: (newSort) => setParams({ sort: newSort === defaultSort ? undefined : newSort }),
    setRange: (newSince, newUntil) =>
      setParams({
        since: newSince === DEFAULT_SINCE ? undefined : newSince,
        until: newUntil || undefined,
      }),
    setLive: (enabled) => setParams({ live: enabled ? "1" : undefined }),
    fetched,
    shownResult,
    canShowMore: () => {
      if (fetched.hasNextPage()) return fetched.pageCount() * limit() < API_MAX_ROWS;
      const body = shownResult()?.body;
      return body !== undefined && "truncated" in body && body.truncated && limit() < API_MAX_ROWS;
    },
    showMore: () => {
      if (fetched.hasNextPage()) fetched.fetchNextPage();
      else {
        setRaisedLimit({ forQuery: queryIdentity(), limit: Math.min(API_MAX_ROWS, limit() * 2) });
      }
    },
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
