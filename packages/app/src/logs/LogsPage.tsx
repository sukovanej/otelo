import { createSignal, Match, Show, Switch } from "solid-js";

import { getLogGroups, getLogs, type LogGroups, type LogLine, type Logs } from "@otelo/api";
import { EmptyMessage } from "@otelo/ui";

import { createListState, formatCount, usePageKeys } from "../list";
import ListContent from "../ListContent";
import QueryBar from "../QueryBar";
import LinePanel from "./LinePanel";
import LogGroupList from "./LogGroupList";
import LogLines, { toLineKey } from "./LogLines";

const VIEWS = [
  { value: "lines", label: "Lines" },
  { value: "groups", label: "Templates" },
] as const;

const NO_LINES_MESSAGE = "No lines in this range match the query.";

type LogsView = "lines" | "groups";

type LogsResult = LinesResult | GroupsResult;

interface LinesResult {
  readonly view: "lines";
  readonly body: Logs;
}

interface GroupsResult {
  readonly view: "groups";
  readonly body: LogGroups;
}

export default function LogsPage() {
  const list = createListState<LogsView, LogsResult>({
    views: ["lines", "groups"],
    firstLimits: { lines: 200, groups: 50 },
    fetch: async (key, signal) =>
      key.view === "groups"
        ? { view: "groups", body: await getLogGroups(key, signal) }
        : { view: "lines", body: await getLogs(key, signal) },
  });

  // The line stays open when a reload or another query no longer brings it.
  const [selectedLine, setSelectedLine] = createSignal<LogLine>();
  const selectedKey = () => {
    const line = selectedLine();
    return line && toLineKey(line);
  };

  let queryInput: HTMLInputElement | undefined;
  usePageKeys({ queryInput: () => queryInput, onEscape: () => setSelectedLine(undefined) });

  const lines = () => {
    const result = list.shownResult();
    return result?.view === "lines" ? result.body : undefined;
  };
  const groups = () => {
    const result = list.shownResult();
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
                {formatCount(body().logs.length, "line")}
                {body().truncated ? ", newest first; more match" : ""}
              </>
            )}
          </Match>
          <Match when={groups()}>
            {(body) => (
              <>
                {formatCount(body().groups.length, "template")} of{" "}
                {formatCount(body().scanned, "line")}
                {body().partial ? " (the newest only)" : ""}
              </>
            )}
          </Match>
        </Switch>
      </QueryBar>

      <ListContent
        list={list}
        signal="logs"
        singularNoun="line"
        panel={
          <Show when={list.view() === "lines" && selectedLine()}>
            {(line) => (
              <LinePanel
                line={line()}
                linksToTrace
                onFilter={list.addTerm}
                onClose={() => setSelectedLine(undefined)}
              />
            )}
          </Show>
        }
      >
        <Switch>
          <Match when={lines()}>
            {(body) => (
              <Show
                when={body().logs.length > 0}
                fallback={<EmptyMessage>{NO_LINES_MESSAGE}</EmptyMessage>}
              >
                <LogLines
                  lines={body().logs}
                  selectedKey={selectedKey()}
                  onSelect={setSelectedLine}
                />
              </Show>
            )}
          </Match>
          <Match when={groups()}>
            {(body) => (
              <Show
                when={body().groups.length > 0}
                fallback={<EmptyMessage>{NO_LINES_MESSAGE}</EmptyMessage>}
              >
                <LogGroupList
                  groups={body().groups}
                  onShowLines={(term) => list.addTerm(term, "lines")}
                />
              </Show>
            )}
          </Match>
        </Switch>
      </ListContent>
    </div>
  );
}
