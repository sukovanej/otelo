import { createEffect, createMemo, createSignal, type JSX, on, onCleanup, Show } from "solid-js";

import { textX } from "../caret";
import { control, cx, plain, type Size, sizes, textInput } from "../classes";
import { applySuggestion, type Suggestion, toChars, toUtf16 } from "../completion";
import { type QueryToken, splitIntoPieces } from "../highlight";
import { listStep, move } from "../keys";
import QueryInputColors from "./query-input-colors";
import QueryInputHelp from "./query-input-help";
import QueryInputSuggestions, {
  SUGGESTION_LIST_ID,
  toSuggestionId,
} from "./query-input-suggestions";

const COMPLETE_DELAY_MS = 60;

const NO_SUGGESTION_PICKED = -1;

type HighlightQuery = (query: string) => QueryToken[];

type CompleteQuery = (
  query: string,
  cursorInChars: number,
  signal: AbortSignal,
) => Promise<Suggestion[]>;

type DrawCard = () => JSX.Element;

type DescribeToken = (
  query: string,
  token: QueryToken,
  signal: AbortSignal,
) => Promise<DrawCard | undefined>;

interface PointedToken {
  readonly token: QueryToken;
  readonly left: number;
}

interface OpenHelp {
  readonly left: number;
  readonly drawCard: DrawCard;
}

interface QueryInputProps {
  readonly value: string;
  readonly highlight: HighlightQuery;
  readonly complete: CompleteQuery;
  readonly help: DescribeToken;
  readonly onInput: (query: string) => void;
  readonly onSubmit: () => void;
  readonly placeholder?: string;
  readonly size?: Size | undefined;
  readonly ref?: (input: HTMLInputElement) => void;
}

export default function QueryInput(props: QueryInputProps) {
  let frame!: HTMLDivElement;
  let input!: HTMLInputElement;
  let pieceElements!: HTMLDivElement;
  let listbox: HTMLUListElement | undefined;
  const [suggestions, setSuggestions] = createSignal<Suggestion[]>([]);
  const [listOpen, setListOpen] = createSignal(false);
  const [activeIndex, setActiveIndex] = createSignal(NO_SUGGESTION_PICKED);
  const [listLeft, setListLeft] = createSignal(0);
  const [scrollLeft, setScrollLeft] = createSignal(0);
  const [openHelp, setOpenHelp] = createSignal<OpenHelp>();
  let completeTimer: ReturnType<typeof setTimeout> | undefined;
  let completeController: AbortController | undefined;
  let helpController: AbortController | undefined;
  let pointedToken: QueryToken | undefined;

  const textClasses = () =>
    cx(control, sizes[props.size ?? "md"], "font-mono", props.size === "lg" && "text-md");
  const coloredPieces = createMemo(() =>
    splitIntoPieces(props.value, props.highlight(props.value)),
  );
  const listShown = () => listOpen() && !openHelp();

  const followScroll = () => setScrollLeft(input.scrollLeft);
  // A value set from outside scrolls the input without an event.
  createEffect(on(() => props.value, followScroll));

  const closeSuggestions = () => {
    clearTimeout(completeTimer);
    completeController?.abort();
    setListOpen(false);
    setActiveIndex(NO_SUGGESTION_PICKED);
  };

  const requestSuggestions = () => {
    clearTimeout(completeTimer);
    completeTimer = setTimeout(() => {
      completeController?.abort();
      const controller = new AbortController();
      completeController = controller;
      const query = input.value;
      const cursorInChars = toChars(query, input.selectionStart ?? query.length);
      props.complete(query, cursorInChars, controller.signal).then(
        (list) => {
          if (completeController !== controller || document.activeElement !== input) return;
          const first = list[0];
          if (first) setListLeft(textX(input, toUtf16(query, first.start)));
          setSuggestions(list);
          setActiveIndex(NO_SUGGESTION_PICKED);
          setListOpen(list.length > 0);
        },
        (error: unknown) => {
          // Completion is a help, so a failed one only hides the list.
          if (!isAbortError(error)) setListOpen(false);
        },
      );
    }, COMPLETE_DELAY_MS);
  };

  const takeSuggestion = (suggestion: Suggestion) => {
    const next = applySuggestion(input.value, suggestion);
    input.value = next.text;
    input.setSelectionRange(next.cursor, next.cursor);
    props.onInput(next.text);
    requestSuggestions();
  };

  // The card stays closed until the pointer comes to another token.
  const dismissHelp = () => {
    helpController?.abort();
    setOpenHelp(undefined);
  };

  const closeHelp = () => {
    dismissHelp();
    pointedToken = undefined;
  };

  const findTokenAt = (clientX: number): PointedToken | undefined => {
    const pieces = coloredPieces();
    for (const [index, element] of Array.from(pieceElements.children).entries()) {
      const box = element.getBoundingClientRect();
      const token = pieces[index]?.token;
      if (token && box.left <= clientX && clientX < box.right) {
        return { token, left: box.left - frame.getBoundingClientRect().left };
      }
    }
    return undefined;
  };

  const onPointerMove = (e: PointerEvent) => {
    const pointed = findTokenAt(e.clientX);
    if (pointed?.token.start === pointedToken?.start && pointed?.token.end === pointedToken?.end) {
      return;
    }
    closeHelp();
    if (!pointed) return;
    pointedToken = pointed.token;
    const controller = new AbortController();
    helpController = controller;
    props.help(props.value, pointed.token, controller.signal).then(
      (drawCard) => {
        if (helpController === controller && drawCard) {
          setOpenHelp({ left: pointed.left, drawCard });
        }
      },
      () => {
        // Help that fails shows nothing.
      },
    );
  };

  const onKeyDown = (e: KeyboardEvent) => {
    dismissHelp();
    const list = suggestions();
    const shown = listOpen() && list.length > 0;
    const step = listStep(e);
    if (step !== 0) {
      e.preventDefault();
      if (!shown) {
        if (step > 0) requestSuggestions();
        return;
      }
      setActiveIndex((index) => move(index, step, list.length, true));
      listbox?.querySelector("[aria-selected=true]")?.scrollIntoView({ block: "nearest" });
      return;
    }
    switch (e.key) {
      case "Tab": {
        const pick = list[Math.max(activeIndex(), 0)];
        if (shown && pick && !e.shiftKey) {
          e.preventDefault();
          takeSuggestion(pick);
        }
        return;
      }
      case "Enter": {
        const pick = list[activeIndex()];
        e.preventDefault();
        if (shown && pick) {
          takeSuggestion(pick);
        } else {
          closeSuggestions();
          props.onSubmit();
        }
        return;
      }
      case "Escape":
        if (shown) {
          e.preventDefault();
          closeSuggestions();
        } else {
          input.blur();
        }
        return;
    }
  };

  onCleanup(closeSuggestions);
  onCleanup(closeHelp);

  return (
    <div ref={frame} class="relative min-w-0 flex-1">
      <input
        ref={(el) => {
          input = el;
          props.ref?.(el);
        }}
        type="text"
        // The input draws the caret and the selection, and QueryInputColors
        // draws its text. A `::selection` that sets a color loses the
        // default background, and the placeholder takes the color of the
        // text, so both are set.
        class={cx(
          textClasses(),
          plain,
          textInput,
          "block w-full text-transparent caret-ink selection:bg-active selection:text-transparent placeholder:text-ink/50",
        )}
        spellcheck={false}
        autocomplete="off"
        placeholder={props.placeholder}
        value={props.value}
        onInput={(e) => {
          closeHelp();
          props.onInput(e.currentTarget.value);
          requestSuggestions();
        }}
        onPointerMove={onPointerMove}
        onPointerLeave={closeHelp}
        onPointerDown={dismissHelp}
        onScroll={() => {
          closeHelp();
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
            requestSuggestions();
          }
        }}
        onClick={requestSuggestions}
        onFocus={requestSuggestions}
        onBlur={closeSuggestions}
        role="combobox"
        aria-expanded={listShown()}
        aria-controls={SUGGESTION_LIST_ID}
        aria-activedescendant={
          listShown() && activeIndex() !== NO_SUGGESTION_PICKED
            ? toSuggestionId(activeIndex())
            : undefined
        }
        aria-autocomplete="list"
      />
      <QueryInputColors
        ref={(el) => (pieceElements = el)}
        class={textClasses()}
        pieces={coloredPieces()}
        scrollLeft={scrollLeft()}
      />
      <Show when={openHelp()}>
        {(help) => <QueryInputHelp left={help().left}>{help().drawCard()}</QueryInputHelp>}
      </Show>
      <Show when={listShown()}>
        <QueryInputSuggestions
          ref={(el) => (listbox = el)}
          suggestions={suggestions()}
          activeIndex={activeIndex()}
          left={listLeft()}
          onTake={takeSuggestion}
          onPoint={setActiveIndex}
        />
      </Show>
    </div>
  );
}

function isAbortError(error: unknown): boolean {
  return error instanceof DOMException && error.name === "AbortError";
}
