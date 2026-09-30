import { type Accessor, createEffect, createSignal, onCleanup } from "solid-js";

/** The room a popover keeps to the right edge of the window. */
const GAP = 8;

export interface Popover {
  open: Accessor<boolean>;
  show: () => void;
  hide: () => void;
  /** Whether the popover lines up with the right edge of its control, since
   * it would leave the window from the left edge. */
  atEnd: Accessor<boolean>;
  /** The `ref` of the element that holds the control and the popover. */
  root: (el: HTMLElement) => void;
  /** The `ref` of the popover. */
  panel: (el: HTMLElement) => void;
}

/**
 * The state of a popover that opens under a control and holds controls of
 * its own. A press outside of it closes it, and so does the focus leaving it
 * and Escape, which gives the focus to `focus`. A popover inside another
 * takes Escape first.
 */
export function createPopover(options: { focus: () => HTMLElement }): Popover {
  const [open, setOpen] = createSignal(false);
  const [atEnd, setAtEnd] = createSignal(false);
  let root: HTMLElement | undefined;
  const hide = () => setOpen(false);

  const onPointerDown = (e: PointerEvent) => {
    if (e.target instanceof Node && !root?.contains(e.target)) hide();
  };
  createEffect(() => {
    if (!open()) return;
    document.addEventListener("pointerdown", onPointerDown, true);
    onCleanup(() => document.removeEventListener("pointerdown", onPointerDown, true));
  });

  return {
    open,
    show: () => {
      setAtEnd(false);
      setOpen(true);
    },
    hide,
    atEnd,
    root: (el) => {
      root = el;
      // Listeners on the element, not the delegated ones of Solid, so that
      // Escape stops here, before a popover around this one and the page.
      el.addEventListener("keydown", (e) => {
        if (e.key !== "Escape" || !open()) return;
        e.preventDefault();
        e.stopPropagation();
        hide();
        options.focus().focus();
      });
      el.addEventListener("focusout", (e) => {
        if (e.relatedTarget instanceof Node && !el.contains(e.relatedTarget)) hide();
      });
    },
    // The popover has its size once it is in the page.
    panel: (el) =>
      queueMicrotask(() => {
        if (el.getBoundingClientRect().right > window.innerWidth - GAP) setAtEnd(true);
      }),
  };
}
