// The state of a page that lists records for a query and a range: the logs
// page and the traces page. The query, the range, the view, and live mode
// live in the URL, so a link opens the same page.

import { useSearchParams } from "@solidjs/router";
import { createEffect, createMemo, createSignal, on, onCleanup } from "solid-js";

import type { ListQuery } from "@siner/api";

import { createFetch, type Fetched } from "./fetch";
import { addTerm } from "./query";

/** The most rows the API returns. */
export const MAX_LIMIT = 10_000;

/** How often a live page reloads. */
export const LIVE_MS = 5_000;

export const DEFAULT_SINCE = "1h";

/** What every list answers besides its rows. */
interface ListBody {
  truncated: boolean;
  unindexed: string[];
}

/** The answer of one view: its name, so a page shows it only in that view,
 * and its body. */
export interface ListResult<V extends string> {
  view: V;
  body: ListBody;
}

export interface List<V extends string, R extends ListResult<V>> {
  q: () => string;
  since: () => string;
  until: () => string;
  view: () => V;
  /** Whether the page reloads by itself, which needs a range that ends now. */
  live: () => boolean;
  /** The query as typed; it runs on Enter. */
  draft: () => string;
  setDraft: (q: string) => void;
  /** Runs the draft, or the query again when it is the same. */
  run: () => void;
  /** Joins `term` to the query with `AND`, and goes to `view` when given. */
  filter: (term: string, view?: V) => void;
  setView: (view: V) => void;
  setRange: (since: string, until: string) => void;
  setLive: (live: boolean) => void;
  fetched: Fetched<R>;
  /** The last answer, when it is of the view the page shows. */
  current: () => R | undefined;
  /** Whether "Show more" can raise the limit. */
  canShowMore: () => boolean;
  showMore: () => void;
}

/**
 * The state of a list with `views`, the first of them the default. `page`
 * is the first limit of each view, and "Show more" doubles it.
 */
export function createList<V extends string, R extends ListResult<V>>(options: {
  views: readonly [V, ...V[]];
  page: Record<V, number>;
  fetch: (key: ListQuery & { view: V }, signal: AbortSignal) => Promise<R>;
}): List<V, R> {
  const [params, setParams] = useSearchParams<{
    q?: string;
    since?: string;
    until?: string;
    view?: string;
    live?: string;
  }>();
  const [first] = options.views;
  const q = () => params.q ?? "";
  const since = () => params.since || DEFAULT_SINCE;
  const until = () => params.until ?? "";
  const view = () => options.views.find((v) => v === params.view) ?? first;
  const live = () => params.live === "1" && until() === "";
  // The default view stays out of the URL.
  const viewParam = (v: V) => (v === first ? undefined : v);

  const [draft, setDraft] = createSignal(q());
  createEffect(on(q, setDraft, { defer: true }));

  // "Show more" raises the limit of one query; any other query starts over.
  const base = () => JSON.stringify([view(), q(), since(), until()]);
  const [more, setMore] = createSignal({ base: "", limit: 0 });
  const limit = () => (more().base === base() ? more().limit : options.page[view()]);

  const key = createMemo(
    () => ({ view: view(), q: q().trim(), since: since(), until: until(), limit: limit() }),
    undefined,
    { equals: (a, b) => JSON.stringify(a) === JSON.stringify(b) },
  );
  const fetched = createFetch(key, options.fetch);

  createEffect(() => {
    if (!live()) return;
    const timer = setInterval(() => {
      if (!fetched.loading()) fetched.reload();
    }, LIVE_MS);
    onCleanup(() => clearInterval(timer));
  });

  const current = () => {
    const result = fetched.data();
    return result?.view === view() ? result : undefined;
  };

  return {
    q,
    since,
    until,
    view,
    live,
    draft,
    setDraft,
    run: () => {
      if (draft() === q()) fetched.reload();
      else setParams({ q: draft() || undefined });
    },
    filter: (term, v = view()) => setParams({ q: addTerm(q(), term), view: viewParam(v) }),
    setView: (v) => setParams({ view: viewParam(v) }),
    setRange: (s, u) =>
      setParams({ since: s === DEFAULT_SINCE ? undefined : s, until: u || undefined }),
    setLive: (enabled) => setParams({ live: enabled ? "1" : undefined }),
    fetched,
    current,
    canShowMore: () => (current()?.body.truncated ?? false) && limit() < MAX_LIMIT,
    showMore: () => setMore({ base: base(), limit: Math.min(MAX_LIMIT, limit() * 2) }),
  };
}

/**
 * `/` focuses the query of a page that has one, and Escape calls `onEscape`,
 * unless the focus is in a field, which takes the key.
 */
export function usePageKeys(keys: {
  query?: () => HTMLInputElement | undefined;
  onEscape: () => void;
}) {
  const onKey = (e: KeyboardEvent) => {
    const typing =
      e.target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName);
    if (typing || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === "/" && keys.query) {
      e.preventDefault();
      keys.query()?.focus();
    } else if (e.key === "Escape") {
      keys.onEscape();
    }
  };
  document.addEventListener("keydown", onKey);
  onCleanup(() => document.removeEventListener("keydown", onKey));
}

export const count = (n: number, noun: string) =>
  `${n.toLocaleString()} ${noun}${n === 1 ? "" : "s"}`;
