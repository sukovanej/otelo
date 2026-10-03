import { createMemo, Match, Show, Switch } from "solid-js";

import type { Attributes } from "@otelo/api";
import { SpanIcon } from "@otelo/icons";
import { LevelBadge, Tooltip } from "@otelo/ui";

import {
  isSqlSystem,
  readSpanMeaning,
  type SpanMeaning,
  stripBadgesFromName,
} from "../../semantics";
import SpanTitleDatabase from "./span-title-database";
import SpanTitleHttp from "./span-title-http";

type SpanTitleProps = SpanTitleOfSpanProps | SpanTitleOfGroupProps;

interface SpanTitleBaseProps {
  readonly name: string;
  readonly attributes: Attributes;
}

interface SpanTitleOfSpanProps extends SpanTitleBaseProps {
  readonly variant: "span";
  readonly error: boolean;
}

interface SpanTitleOfGroupProps extends SpanTitleBaseProps {
  readonly variant: "group";
}

export default function SpanTitle(props: SpanTitleProps) {
  const meaning = createMemo((): SpanMeaning => {
    const spanMeaning = readSpanMeaning(props.attributes);
    return spanMeaning.kind === "http" && props.variant === "group"
      ? { ...spanMeaning, status: undefined }
      : spanMeaning;
  });
  const remainingName = () => stripBadgesFromName(props.name, meaning());
  const httpMeaning = () => {
    const spanMeaning = meaning();
    return spanMeaning.kind === "http" ? spanMeaning : undefined;
  };
  const databaseMeaning = () => {
    const spanMeaning = meaning();
    return spanMeaning.kind === "database" ? spanMeaning : undefined;
  };
  // The tip of a SQL query takes the place of the one of the browser.
  const title = () => {
    const call = databaseMeaning();
    return call && isSqlSystem(call.system) ? undefined : props.name;
  };
  return (
    <span class="flex min-w-0 items-baseline gap-1.5 overflow-hidden" title={title()}>
      <Show when={props.variant === "span" && props.error}>
        <LevelBadge level="ERROR" />
      </Show>
      <Switch
        fallback={
          <>
            <Tooltip content="Span" class="self-center">
              <SpanIcon class="text-muted" title="Span" />
            </Tooltip>
            <span class="truncate">{props.name}</span>
          </>
        }
      >
        <Match when={httpMeaning()}>
          {(http) => <SpanTitleHttp meaning={http()} remainingName={remainingName()} />}
        </Match>
        <Match when={databaseMeaning()}>
          {(database) => <SpanTitleDatabase meaning={database()} name={props.name} />}
        </Match>
      </Switch>
    </span>
  );
}
