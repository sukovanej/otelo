import { Show } from "solid-js";

import { DatabaseSystemIcon, toDatabaseName } from "@otelo/icons";
import { SqlCode, SqlStart, Tooltip } from "@otelo/ui";

import { type DatabaseSpan, isSqlSystem, toDatabaseIconId } from "../../semantics";

const TOOLTIP_MAX_LINES = 24;

interface SpanTitleDatabaseProps {
  readonly meaning: DatabaseSpan;
  readonly name: string;
}

export default function SpanTitleDatabase(props: SpanTitleDatabaseProps) {
  const iconId = () => toDatabaseIconId(props.meaning.system);
  const system = () => toDatabaseName(iconId());
  const queryOrName = () => props.meaning.query ?? props.name;
  return (
    <>
      <Tooltip content={`Database call to ${system()}`} class="self-center">
        <DatabaseSystemIcon system={iconId()} title={`Database call to ${system()}`} />
      </Tooltip>
      <Show
        when={isSqlSystem(props.meaning.system)}
        fallback={
          <span class="truncate" title={queryOrName()}>
            {queryOrName()}
          </span>
        }
      >
        <Tooltip
          rich
          content={<SqlCode text={queryOrName()} maxLines={TOOLTIP_MAX_LINES} />}
          class="min-w-0"
        >
          <span class="truncate">
            <SqlStart text={queryOrName()} />
          </span>
        </Tooltip>
      </Show>
    </>
  );
}
