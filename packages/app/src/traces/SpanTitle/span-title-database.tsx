import { Show } from "solid-js";

import { DatabaseSystemIcon, toDatabaseName } from "@otelo/icons";
import { Tooltip } from "@otelo/ui";

import { type DatabaseSpan, isSqlSystem, toDatabaseIconId } from "../../semantics";
import SqlQueryStart from "../../SqlQueryStart";

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
        <SqlQueryStart query={queryOrName()} />
      </Show>
    </>
  );
}
