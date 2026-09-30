import { createEffect, createSignal, createUniqueId, type JSX, onCleanup, Show } from "solid-js";
import { Portal } from "solid-js/web";

/** How far under the tip of the pointer the tip starts, past the arrow. */
const CURSOR = 18;

/** The gap between the tip and what it points at, and between the tip and
 * the edges of the window. */
const GAP = 6;

/** Where a tip goes: centered on `x`, with its top at `below`, or with its
 * bottom at `above` when the window has no room under `below`. */
interface Anchor {
  x: number;
  below: number;
  above: number;
}

const plain = "bg-ink px-2 py-1 font-sans whitespace-nowrap text-surface";

const rich =
  "max-h-[80vh] max-w-[min(44rem,90vw)] overflow-hidden border border-line bg-surface px-2.5 py-2 text-ink";

/**
 * A short text that shows over `children` at once while the pointer is on
 * them, under the pointer and following it, or above it when there is no
 * room under it. When the focus moves into `children`, it shows under them.
 * It stays inside the window, and renders in the open modal that holds
 * `children`, or else in the body, so no panel clips it.
 */
export default function Tooltip(props: {
  content: JSX.Element;
  /** Whether `content` is more than a short text, such as code in its
   * colors: the tip is then a popup on the surface, which wraps what is
   * wider than it and cuts what is taller than the window. */
  rich?: boolean;
  /** Classes for the element around `children`, which is an inline flex box. */
  class?: string;
  children: JSX.Element;
}) {
  const id = createUniqueId();
  const [anchor, setAnchor] = createSignal<Anchor>();
  const [tip, setTip] = createSignal<HTMLDivElement>();
  let trigger!: HTMLSpanElement;

  const atPointer = (e: PointerEvent) =>
    setAnchor({ x: e.clientX, below: e.clientY + CURSOR, above: e.clientY - GAP });
  const atElement = () => {
    const box = trigger.getBoundingClientRect();
    setAnchor({ x: box.left + box.width / 2, below: box.bottom + GAP, above: box.top - GAP });
  };
  const hide = () => setAnchor(undefined);
  onCleanup(hide);

  // Places the tip once it is in the page, since that needs its size.
  createEffect(() => {
    const at = anchor();
    const el = tip();
    if (!at || !el) return;
    const { offsetWidth: width, offsetHeight: height } = el;
    const left = Math.min(Math.max(at.x - width / 2, GAP), window.innerWidth - width - GAP);
    const top = at.below + height + GAP <= window.innerHeight ? at.below : at.above - height;
    el.style.left = `${left}px`;
    el.style.top = `${Math.max(top, GAP)}px`;
  });

  return (
    <>
      <span
        ref={trigger}
        class={`inline-flex ${props.class ?? ""}`}
        aria-describedby={anchor() ? id : undefined}
        onPointerEnter={atPointer}
        onPointerMove={atPointer}
        onPointerLeave={hide}
        onFocusIn={atElement}
        onFocusOut={hide}
      >
        {props.children}
      </span>
      <Show when={anchor()}>
        <Portal mount={trigger.closest("dialog[open]") ?? document.body}>
          <div
            ref={setTip}
            id={id}
            role="tooltip"
            class={`pointer-events-none fixed top-0 left-0 z-50 rounded-md text-xs shadow-popup ${props.rich ? rich : plain}`}
          >
            {props.content}
          </div>
        </Portal>
      </Show>
    </>
  );
}
