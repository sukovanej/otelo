import {
  createEffect,
  createMemo,
  createSignal,
  For,
  Index,
  type JSX,
  on,
  onCleanup,
  Show,
} from "solid-js";

import { textX } from "./caret";
import {
  activeOption,
  control,
  cx,
  option,
  plain,
  popover,
  popup,
  type Size,
  sizes,
  textInput,
} from "./classes";
import { applySuggestion, type Suggestion, toChars, toUtf16 } from "./completion";
import { type Highlight, pieceClass, pieces, type QueryToken } from "./highlight";
import { listStep, move } from "./keys";

/** How long typing pauses before the input asks for completions. */
const COMPLETE_DELAY_MS = 60;

/** The width of the list of suggestions, in pixels. */
const LIST_WIDTH = 360;

/** The width of the card of help, in pixels. */
const HELP_WIDTH = 360;

/** Asks for the help of `token` of `query`: what draws its card, or nothing
 * for a token that has no help. */
export type Help = (
  query: string,
  token: QueryToken,
  signal: AbortSignal,
) => Promise<(() => JSX.Element) | undefined>;

/** Asks for what fits at `cursor`, in characters, of `query`. */
export type Complete = (
  query: string,
  cursor: number,
  signal: AbortSignal,
) => Promise<Suggestion[]>;

const aborted = (error: unknown) => error instanceof DOMException && error.name === "AbortError";

/**
 * A query input that colors the query by its tokens and suggests what fits at
 * the cursor, in a list that opens under the text it would replace. The arrow
 * keys, or Ctrl+J and Ctrl+K, pick a suggestion, Tab or Enter takes it, and
 * Enter without a picked suggestion runs the query.
 *
 * The input draws the caret and the selection, and its own text is
 * transparent: a layer over it draws the same text in color. The pointer on
 * a token opens the card of its help under it at once, in place of the list
 * of suggestions, until the pointer leaves the token or a key is pressed.
 */
export default function QueryInput(props: {
  value: string;
  highlight: Highlight;
  complete: Complete;
  help: Help;
  onInput: (q: string) => void;
  onSubmit: () => void;
  placeholder?: string;
  size?: Size | undefined;
  ref?: (input: HTMLInputElement) => void;
}) {
  let frame!: HTMLDivElement;
  let input!: HTMLInputElement;
  // The colored text: one element for each piece of `colored`.
  let layer!: HTMLDivElement;
  let listbox: HTMLUListElement | undefined;
  const [suggestions, setSuggestions] = createSignal<Suggestion[]>([]);
  const [open, setOpen] = createSignal(false);
  const [active, setActive] = createSignal(-1);
  // Where the list opens: under the start of the text a suggestion replaces.
  const [left, setLeft] = createSignal(0);
  // How far a query longer than the input is scrolled, which the colored
  // text follows.
  const [scrollLeft, setScrollLeft] = createSignal(0);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let controller: AbortController | undefined;

  const colored = createMemo(() => pieces(props.value, props.highlight(props.value)));
  const followScroll = () => setScrollLeft(input.scrollLeft);
  // A value set from outside scrolls the input without an event.
  createEffect(on(() => props.value, followScroll));

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

  // The card of the token the pointer is on, and where it opens.
  const [help, setHelp] = createSignal<{ left: number; card: () => JSX.Element }>();
  let hovered: QueryToken | undefined;
  let helpController: AbortController | undefined;

  // Closes the card until the pointer comes to another token.
  const dismissHelp = () => {
    helpController?.abort();
    setHelp(undefined);
  };

  const hideHelp = () => {
    dismissHelp();
    hovered = undefined;
  };

  // The card takes the place of the list while it is open.
  const listShown = () => open() && !help();

  /** The token drawn at the horizontal position `x` of the window, and its
   * left edge in the frame. */
  const tokenAt = (x: number) => {
    const list = colored();
    for (const [i, span] of Array.from(layer.children).entries()) {
      const box = span.getBoundingClientRect();
      const token = list[i]?.token;
      if (token && box.left <= x && x < box.right) {
        return { token, left: box.left - frame.getBoundingClientRect().left };
      }
    }
    return undefined;
  };

  const onPointerMove = (e: PointerEvent) => {
    const at = tokenAt(e.clientX);
    if (at?.token.start === hovered?.start && at?.token.end === hovered?.end) return;
    hideHelp();
    if (!at) return;
    hovered = at.token;
    const current = new AbortController();
    helpController = current;
    props.help(props.value, at.token, current.signal).then(
      (draw) => {
        if (helpController === current && draw) setHelp({ left: at.left, card: draw });
      },
      () => {
        // Help that fails shows nothing.
      },
    );
  };

  const take = (suggestion: Suggestion) => {
    const next = applySuggestion(input.value, suggestion);
    input.value = next.text;
    input.setSelectionRange(next.cursor, next.cursor);
    props.onInput(next.text);
    suggest();
  };

  const onKeyDown = (e: KeyboardEvent) => {
    dismissHelp();
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
  onCleanup(hideHelp);

  return (
    <div ref={frame} class="relative min-w-0 flex-1">
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
          "block w-full font-mono text-transparent caret-ink selection:bg-active selection:text-transparent placeholder:text-ink/50",
          props.size === "lg" && "text-md",
        )}
        spellcheck={false}
        autocomplete="off"
        placeholder={props.placeholder}
        value={props.value}
        onInput={(e) => {
          hideHelp();
          props.onInput(e.currentTarget.value);
          suggest();
        }}
        onPointerMove={onPointerMove}
        onPointerLeave={hideHelp}
        onPointerDown={dismissHelp}
        onScroll={() => {
          hideHelp();
          followScroll();
        }}
        onSelect={followScroll}
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
        aria-expanded={listShown()}
        aria-controls="query-suggestions"
        aria-activedescendant={
          listShown() && active() >= 0 ? `query-suggestion-${active()}` : undefined
        }
        aria-autocomplete="list"
      />
      <div
        aria-hidden="true"
        class={cx(
          control,
          sizes[props.size ?? "md"],
          "pointer-events-none absolute inset-0 border-transparent font-mono",
          props.size === "lg" && "text-md",
        )}
      >
        {/* Cuts the text at the padding, as the input does. */}
        <div class="flex h-full items-center overflow-hidden">
          <div
            ref={layer}
            class="whitespace-pre"
            style={{ transform: `translateX(${-scrollLeft()}px)` }}
          >
            <Index each={colored()}>
              {(piece) => <span class={pieceClass(piece())}>{piece().text}</span>}
            </Index>
          </div>
        </div>
      </div>
      <Show when={help()}>
        {(shown) => (
          <div
            role="tooltip"
            class={cx(popover, "pointer-events-none max-w-full p-3")}
            // Moved left by the card's border and padding, so its text lines
            // up with the token.
            style={{
              width: `${HELP_WIDTH}px`,
              left: `max(0px, min(${shown().left}px - 13px, 100% - ${HELP_WIDTH}px))`,
            }}
          >
            {shown().card()}
          </div>
        )}
      </Show>
      <Show when={listShown()}>
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
