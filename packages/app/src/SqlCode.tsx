import { For, Show } from "solid-js";

import { type SqlKind, sqlLines, type SqlToken, sqlTokens } from "./sql";

const kindClasses: Record<SqlKind, string | undefined> = {
  keyword: "text-database",
  string: "text-success",
  number: "text-warn",
  placeholder: "text-placeholder",
  comment: "text-muted",
  punctuation: "text-muted",
  space: undefined,
  text: undefined,
};

/** How many characters of a query a row reads: more than a row shows. */
const START_CHARS = 300;

/** How many spaces a level of a query indents its lines by. */
const INDENT = 2;

function Tokens(props: { tokens: SqlToken[] }) {
  return (
    <For each={props.tokens}>
      {(token) => <span class={kindClasses[token.kind]}>{token.text}</span>}
    </For>
  );
}

/** The start of a SQL query in its colors, as it came, for a row that shows
 * one line and truncates it. */
export function SqlStart(props: { text: string }) {
  return <Tokens tokens={sqlTokens(props.text.slice(0, START_CHARS))} />;
}

/**
 * A SQL query in its colors, laid out as `sqlLines` does, in the mono font.
 * A line that wraps goes on a level deeper than it starts. With `max`, it
 * shows that many lines at most, and says how many more there are.
 */
export default function SqlCode(props: { text: string; max?: number }) {
  const lines = () => sqlLines(props.text);
  const more = () => Math.max(lines().length - (props.max ?? Infinity), 0);
  return (
    <div class="font-mono wrap-anywhere whitespace-pre-wrap">
      <For each={more() > 0 ? lines().slice(0, props.max) : lines()}>
        {(line) => {
          const hang = `${(line.indent + 1) * INDENT}ch`;
          return (
            <div style={{ "padding-left": hang, "text-indent": `-${hang}` }}>
              {" ".repeat(line.indent * INDENT)}
              <Tokens tokens={line.tokens} />
            </div>
          );
        }}
      </For>
      <Show when={more() > 0}>
        <div class="font-sans text-muted">
          {more()} more {more() === 1 ? "line" : "lines"}
        </div>
      </Show>
    </div>
  );
}
