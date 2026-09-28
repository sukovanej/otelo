import { databaseName, DatabaseSystemIcon, GlobeIcon, SpanIcon } from "@siner/icons";
import { Badge, Level, type Tone, Tooltip } from "@siner/ui";
import { Match, Show, Switch } from "solid-js";
import { databaseId, type DbSpan, type HttpSpan, nameRest, spanMeaning } from "../semantics";

const methodTones: Record<string, Tone> = {
  GET: "info",
  POST: "success",
  PUT: "warn",
  PATCH: "warn",
  DELETE: "error",
};

const statusTone = (status: number): Tone => {
  if (status >= 500) return "error";
  if (status >= 400) return "warn";
  if (status >= 300) return "info";
  if (status >= 200) return "success";
  return "muted";
};

function Http(props: { meaning: HttpSpan; rest: string }) {
  return (
    <>
      <Tooltip content="HTTP request" class="self-center">
        <GlobeIcon class="text-muted" title="HTTP request" />
      </Tooltip>
      <Show when={props.meaning.method}>
        {(method) => <Badge tone={methodTones[method()] ?? "muted"}>{method()}</Badge>}
      </Show>
      <Show when={props.meaning.route}>{(route) => <span class="truncate">{route()}</span>}</Show>
      <Show when={props.meaning.status}>
        {(status) => <Badge tone={statusTone(status())}>{status()}</Badge>}
      </Show>
      <Show when={props.rest}>
        <span class="truncate text-muted">{props.rest}</span>
      </Show>
    </>
  );
}

function Db(props: { meaning: DbSpan; rest: string }) {
  const id = () => databaseId(props.meaning.system);
  const system = () => databaseName(id());
  return (
    <>
      <Tooltip content={`Database call to ${system()}`} class="self-center">
        <DatabaseSystemIcon system={id()} title={`Database call to ${system()}`} />
      </Tooltip>
      <Show when={props.meaning.operation}>
        {(operation) => <Badge tone="database">{operation()}</Badge>}
      </Show>
      {/* A name that the badge already says gives way to the query. */}
      <Show
        when={props.rest}
        fallback={
          <Show when={props.meaning.query}>
            {(query) => (
              <span class="truncate text-muted" title={query()}>
                {query()}
              </span>
            )}
          </Show>
        }
      >
        <span class="truncate">{props.rest}</span>
      </Show>
    </>
  );
}

/**
 * The name of a span, told by what it is. An HTTP request shows its method,
 * route, and status, each on a badge. A database call shows the icon of its
 * system and the keyword of its query on a badge. Any other span shows its
 * name. Each starts with an icon of what it is, so the names of a list line
 * up. The words of the name that the badges say are left out. A failed span
 * starts with ERROR.
 */
export default function SpanTitle(props: {
  name: string;
  attributes: Record<string, unknown>;
  error: boolean;
}) {
  const meaning = () => spanMeaning(props.attributes);
  const rest = () => nameRest(props.name, meaning());
  const http = () => {
    const m = meaning();
    return m.type === "http" ? m : undefined;
  };
  const db = () => {
    const m = meaning();
    return m.type === "db" ? m : undefined;
  };
  return (
    <span class="flex min-w-0 items-baseline gap-1.5 overflow-hidden" title={props.name}>
      <Show when={props.error}>
        <Level level="ERROR" />
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
        <Match when={http()}>{(shown) => <Http meaning={shown()} rest={rest()} />}</Match>
        <Match when={db()}>{(shown) => <Db meaning={shown()} rest={rest()} />}</Match>
      </Switch>
    </span>
  );
}
