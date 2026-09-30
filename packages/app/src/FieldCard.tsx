import { For, type JSX, Show } from "solid-js";

import { complete, type FieldBody, type Signal } from "@otelo/api";
import { type QueryToken, toChars } from "@otelo/ui";

import { SOURCES, typeLine, valuesTitle } from "./fieldHelp";

/**
 * Asks the daemon what the field that `token` of `query` names holds, and
 * answers with what draws its card: nothing for a token that is no field, and
 * for a field the daemon does not know. A word still being typed has a card
 * when it is the name of a field already.
 */
export async function fieldHelp(
  kind: Signal,
  query: string,
  token: QueryToken,
  signal: AbortSignal,
): Promise<(() => JSX.Element) | undefined> {
  if (token.kind !== "field" && token.kind !== "word") return undefined;
  // The field alone, so the rest of the query cannot hide it.
  const name = query.slice(token.start, token.end);
  const { field } = await complete(kind, name, toChars(name, name.length), signal);
  return field ? () => <FieldCard field={field} signal={kind} /> : undefined;
}

/** What a field of a query holds: where it comes from, its type, what it
 * means, and its values. */
function FieldCard(props: { field: FieldBody; signal: Signal }) {
  return (
    <div class="flex flex-col gap-2 text-sm">
      <div>
        <div class="flex items-baseline gap-2.5">
          <span class="truncate font-mono font-semibold">{props.field.name}</span>
          <span class="ml-auto whitespace-nowrap text-2xs text-muted">
            {SOURCES[props.field.source]}
          </span>
        </div>
        <div class="text-muted">{typeLine(props.field, props.signal)}</div>
      </div>
      <Show when={props.field.description}>{(description) => <p>{description()}</p>}</Show>
      <Show when={props.field.values.length > 0}>
        <div>
          <div class="text-2xs text-muted">{valuesTitle(props.field)}</div>
          <ul>
            <For each={props.field.values}>
              {(value) => (
                <li class="flex items-baseline gap-2.5">
                  <span class="truncate whitespace-pre font-mono">{value.text}</span>
                  <span class="ml-auto whitespace-nowrap text-2xs text-muted">
                    {value.count?.toLocaleString()}
                  </span>
                </li>
              )}
            </For>
          </ul>
        </div>
      </Show>
    </div>
  );
}
