import { For } from "solid-js";

import { activeOption, cx, option, popup } from "../classes";
import type { Suggestion } from "../completion";

export const SUGGESTION_LIST_ID = "query-suggestions";

const LIST_WIDTH_PX = 360;

interface QueryInputSuggestionsProps {
  readonly suggestions: ReadonlyArray<Suggestion>;
  readonly activeIndex: number;
  readonly left: number;
  readonly onTake: (suggestion: Suggestion) => void;
  readonly onPoint: (index: number) => void;
  readonly ref: (listbox: HTMLUListElement) => void;
}

export default function QueryInputSuggestions(props: QueryInputSuggestionsProps) {
  return (
    <ul
      ref={props.ref}
      class={cx(popup, "max-w-full")}
      id={SUGGESTION_LIST_ID}
      role="listbox"
      // Moved left by its border and padding, so its text lines up with the
      // text it replaces.
      style={{
        width: `${LIST_WIDTH_PX}px`,
        left: `max(0px, min(${props.left}px - 13px, 100% - ${LIST_WIDTH_PX}px))`,
      }}
    >
      <For each={props.suggestions}>
        {(suggestion, index) => (
          <li
            role="option"
            id={toSuggestionId(index())}
            aria-selected={props.activeIndex === index() ? "true" : "false"}
            class={cx(
              option,
              "items-baseline gap-2.5",
              props.activeIndex === index() && activeOption,
            )}
            // Keeps the focus in the input.
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => props.onTake(suggestion)}
            onMouseEnter={() => props.onPoint(index())}
          >
            <span class="truncate whitespace-pre font-mono">{suggestion.text}</span>
            <span class="text-2xs text-muted">{suggestion.kind}</span>
            <span class="ml-auto whitespace-nowrap text-2xs text-muted">{suggestion.detail}</span>
          </li>
        )}
      </For>
    </ul>
  );
}

export function toSuggestionId(index: number): string {
  return `query-suggestion-${index}`;
}
