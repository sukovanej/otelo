import { createMemo, For, Show } from "solid-js";

import { layOutSql } from "./sql";
import SqlTokens from "./sql-tokens";

const INDENT_SPACES = 2;

interface SqlCodeProps {
  readonly text: string;
  readonly maxLines?: number;
}

export default function SqlCode(props: SqlCodeProps) {
  const lines = createMemo(() => layOutSql(props.text));
  const hiddenLineCount = () => Math.max(lines().length - (props.maxLines ?? Infinity), 0);
  return (
    <div class="font-mono wrap-anywhere whitespace-pre-wrap">
      <For each={hiddenLineCount() > 0 ? lines().slice(0, props.maxLines) : lines()} keyed={false}>
        {(line) => {
          const hangingIndent = () => `${(line().indentLevel + 1) * INDENT_SPACES}ch`;
          return (
            <div style={{ "padding-left": hangingIndent(), "text-indent": `-${hangingIndent()}` }}>
              {" ".repeat(line().indentLevel * INDENT_SPACES)}
              <SqlTokens tokens={line().tokens} />
            </div>
          );
        }}
      </For>
      <Show when={hiddenLineCount() > 0}>
        <div class="font-sans text-muted">
          {hiddenLineCount()} more {hiddenLineCount() === 1 ? "line" : "lines"}
        </div>
      </Show>
    </div>
  );
}
