import { createSignal, Match, Show, Switch } from "solid-js";

import { getLogGroups, getLogs, type LogGroups, type LogLine, type Logs } from "@siner/api";

import { count, createList, usePageKeys } from "../list";
import { Empty, ListContent, QueryBar } from "../ListFrame";
import LinePanel from "./LinePanel";
import LogGroupList from "./LogGroupList";
import LogLines, { lineKey } from "./LogLines";

type View = "lines" | "groups";

const VIEWS = [
  { value: "lines", label: "Lines" },
  { value: "groups", label: "Templates" },
] as const;

type Result = { view: "lines"; body: Logs } | { view: "groups"; body: LogGroups };

/**
 * Log lines or their message templates for a query and a range. The query,
 * the range, the view, and live mode live in the URL, so a link opens the
 * same page.
 */
export default function LogsPage() {
  const list = createList<View, Result>({
    views: ["lines", "groups"],
    page: { lines: 200, groups: 50 },
    fetch: async (k, signal) =>
      k.view === "groups"
        ? { view: "groups", body: await getLogGroups(k, signal) }
        : { view: "lines", body: await getLogs(k, signal) },
  });

  // The line open in the panel. It stays open when a reload or another
  // query no longer brings it.
  const [selected, setSelected] = createSignal<LogLine>();
  const selectedKey = () => {
    const line = selected();
    return line && lineKey(line);
  };

  let queryInput: HTMLInputElement | undefined;
  usePageKeys({ query: () => queryInput, onEscape: () => setSelected(undefined) });

  const lines = () => {
    const result = list.current();
    return result?.view === "lines" ? result.body : undefined;
  };
  const groups = () => {
    const result = list.current();
    return result?.view === "groups" ? result.body : undefined;
  };

  return (
    <div class="flex min-h-0 flex-1 flex-col">
      <QueryBar
        list={list}
        signal="logs"
        placeholder='level >= warn service = "api" body ~ "payment failed"'
        views={VIEWS}
        ref={(el) => (queryInput = el)}
      >
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
      </QueryBar>

      <ListContent
        list={list}
        signal="logs"
        noun="line"
        panel={
          <Show when={list.view() === "lines" && selected()}>
            {(line) => (
              <LinePanel
                line={line()}
                onFilter={list.filter}
                onClose={() => setSelected(undefined)}
              />
            )}
          </Show>
        }
      >
        <Switch>
          <Match when={lines()}>
            {(body) => (
              <Show when={body().logs.length > 0} fallback={<Empty>{NO_LINES}</Empty>}>
                <LogLines lines={body().logs} selected={selectedKey()} onSelect={setSelected} />
              </Show>
            )}
          </Match>
          <Match when={groups()}>
            {(body) => (
              <Show when={body().groups.length > 0} fallback={<Empty>{NO_LINES}</Empty>}>
                <LogGroupList
                  groups={body().groups}
                  onShowLines={(term) => list.filter(term, "lines")}
                />
              </Show>
            )}
          </Match>
        </Switch>
      </ListContent>
    </div>
  );
}

const NO_LINES = "No lines in this range match the query.";
