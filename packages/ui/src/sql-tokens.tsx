import { For } from "solid-js";

import type { SqlToken, SqlTokenKind } from "./sql";

const SQL_TOKEN_CLASSES: Record<SqlTokenKind, string | undefined> = {
  keyword: "text-database",
  string: "text-success",
  number: "text-warn",
  placeholder: "text-placeholder",
  comment: "text-muted",
  punctuation: "text-muted",
  space: undefined,
  text: undefined,
};

interface SqlTokensProps {
  readonly tokens: ReadonlyArray<SqlToken>;
}

export default function SqlTokens(props: SqlTokensProps) {
  return (
    <For each={props.tokens} keyed={false}>
      {(token) => <span class={SQL_TOKEN_CLASSES[token().kind]}>{token().text}</span>}
    </For>
  );
}
