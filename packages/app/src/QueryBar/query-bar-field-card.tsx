import { For, Show } from "solid-js";

import type { FieldBody, Signal } from "@otelo/api";

import { formatTypeLine, formatValuesTitle, SOURCE_LABELS } from "../fieldHelp";

interface QueryBarFieldCardProps {
  readonly field: FieldBody;
  readonly signal: Signal;
}

export default function QueryBarFieldCard(props: QueryBarFieldCardProps) {
  return (
    <div class="flex flex-col gap-2 text-sm">
      <div>
        <div class="flex items-baseline gap-2.5">
          <span class="truncate font-mono font-semibold">{props.field.name}</span>
          <span class="ml-auto whitespace-nowrap text-2xs text-muted">
            {SOURCE_LABELS[props.field.source]}
          </span>
        </div>
        <div class="text-muted">{formatTypeLine(props.field, props.signal)}</div>
      </div>
      <Show when={props.field.description}>{(description) => <p>{description()}</p>}</Show>
      <Show when={props.field.values.length > 0}>
        <div>
          <div class="text-2xs text-muted">{formatValuesTitle(props.field)}</div>
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
