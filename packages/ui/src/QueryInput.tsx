import { createSignal, For, onCleanup, Show } from "solid-js";

import { textX } from "./caret";
import {
  activeOption,
  control,
  cx,
  option,
  plain,
  popup,
  type Size,
  sizes,
  textInput,
} from "./classes";
import { applySuggestion, type Suggestion, toChars, toUtf16 } from "./completion";
import { listStep, move } from "./keys";

/** How long typing pauses before the input asks for completions. */
const COMPLETE_DELAY_MS = 60;

/** The width of the list of suggestions, in pixels. */
const LIST_WIDTH = 360;

/** Asks for what fits at `cursor`, in characters, of `query`. */
export type Complete = (
  query: string,
  cursor: number,
  signal: AbortSignal,
) => Promise<Suggestion[]>;

const aborted = (error: unknown) => error instanceof DOMException && error.name === "AbortError";

/**
 * A query input that suggests what fits at the cursor, in a list that opens
 * under the text it would replace. The arrow keys, or Ctrl+J and Ctrl+K, pick
 * a suggestion, Tab or Enter takes it, and Enter without a picked suggestion
 * runs the query.
 */
export default function QueryInput(props: {
  value: string;
  complete: Complete;
  onInput: (q: string) => void;
  onSubmit: () => void;
  placeholder?: string;
  size?: Size;
  ref?: (input: HTMLInputElement) => void;
}) {
  let input!: HTMLInputElement;
  let listbox: HTMLUListElement | undefined;
  const [suggestions, setSuggestions] = createSignal<Suggestion[]>([]);
  const [open, setOpen] = createSignal(false);
  const [active, setActive] = createSignal(-1);
  // Where the list opens: under the start of the text a suggestion replaces.
  const [left, setLeft] = createSignal(0);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let controller: AbortController | undefined;

  const close = () => {
    clearTimeout(timer);
    controller?.abort();
    setOpen(false);
    setActive(-1);
  };

  const suggest = () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      controller?.abort();
      const current = new AbortController();
      controller = current;
      const q = input.value;
      const cursor = toChars(q, input.selectionStart ?? q.length);
      props.complete(q, cursor, current.signal).then(
        (list) => {
          if (controller !== current || document.activeElement !== input) return;
          const first = list[0];
          if (first) setLeft(textX(input, toUtf16(q, first.start)));
          setSuggestions(list);
          setActive(-1);
          setOpen(list.length > 0);
        },
        (e: unknown) => {
          // Completion is a help; a failed one only hides the list.
          if (!aborted(e)) setOpen(false);
        },
      );
    }, COMPLETE_DELAY_MS);
  };

  const take = (suggestion: Suggestion) => {
    const next = applySuggestion(input.value, suggestion);
    input.value = next.text;
    input.setSelectionRange(next.cursor, next.cursor);
    props.onInput(next.text);
    suggest();
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const list = suggestions();
    const shown = open() && list.length > 0;
    const step = listStep(e);
    if (step !== 0) {
      e.preventDefault();
      if (!shown) {
        if (step > 0) suggest();
        return;
      }
      // -1 is the query as typed, above the first suggestion.
      setActive((i) => move(i, step, list.length, true));
      listbox?.querySelector("[aria-selected=true]")?.scrollIntoView({ block: "nearest" });
      return;
    }
    switch (e.key) {
      case "Tab": {
        const pick = list[Math.max(active(), 0)];
        if (shown && pick && !e.shiftKey) {
          e.preventDefault();
          take(pick);
        }
        return;
      }
      case "Enter": {
        const pick = list[active()];
        e.preventDefault();
        if (shown && pick) {
          take(pick);
        } else {
          close();
          props.onSubmit();
        }
        return;
      }
      case "Escape":
        if (shown) {
          e.preventDefault();
          close();
        } else {
          input.blur();
        }
        return;
    }
  };

  onCleanup(close);

  return (
    <div class="relative min-w-0 flex-1">
      <input
        ref={(el) => {
          input = el;
          props.ref?.(el);
        }}
        type="text"
        class={cx(
          control,
          plain,
          sizes[props.size ?? "md"],
          textInput,
          "w-full font-mono",
          props.size === "lg" && "text-md",
        )}
        spellcheck={false}
        autocomplete="off"
        placeholder={props.placeholder}
        value={props.value}
        onInput={(e) => {
          props.onInput(e.currentTarget.value);
          suggest();
        }}
        onKeyDown={onKeyDown}
        onKeyUp={(e) => {
          if (
            e.key === "ArrowLeft" ||
            e.key === "ArrowRight" ||
            e.key === "Home" ||
            e.key === "End"
          ) {
            suggest();
          }
        }}
        onClick={suggest}
        onFocus={suggest}
        onBlur={close}
        role="combobox"
        aria-expanded={open()}
        aria-controls="query-suggestions"
        aria-activedescendant={open() && active() >= 0 ? `query-suggestion-${active()}` : undefined}
        aria-autocomplete="list"
      />
      <Show when={open()}>
        <ul
          ref={listbox}
          class={cx(popup, "max-w-full")}
          id="query-suggestions"
          role="listbox"
          // Moved left by the list's border and padding, so its text lines up
          // with the text it replaces.
          style={{
            width: `${LIST_WIDTH}px`,
            left: `max(0px, min(${left()}px - 13px, 100% - ${LIST_WIDTH}px))`,
          }}
        >
          <For each={suggestions()}>
            {(suggestion, i) => (
              <li
                role="option"
                id={`query-suggestion-${i()}`}
                aria-selected={active() === i()}
                class={cx(option, "items-baseline gap-2.5", active() === i() && activeOption)}
                // Keep the focus in the input.
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => take(suggestion)}
                onMouseEnter={() => setActive(i())}
              >
                <span class="truncate whitespace-pre font-mono">{suggestion.text}</span>
                <span class="text-2xs text-muted">{suggestion.kind}</span>
                <span class="ml-auto whitespace-nowrap text-2xs text-muted">
                  {suggestion.detail}
                </span>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}
