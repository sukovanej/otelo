import { type Accessor, createEffect, createSignal } from "solid-js";

const WINDOW_EDGE_GAP_PX = 8;

interface Popover {
  readonly open: Accessor<boolean>;
  readonly show: () => void;
  readonly hide: () => void;
  readonly alignsRight: Accessor<boolean>;
  readonly rootRef: (root: HTMLElement) => void;
  readonly panelRef: (panel: HTMLElement) => void;
}

interface PopoverOptions {
  readonly returnFocusTo: () => HTMLElement;
}

export function createPopover(options: PopoverOptions): Popover {
  const [open, setOpen] = createSignal(false);
  const [alignsRight, setAlignsRight] = createSignal(false);
  let root: HTMLElement | undefined;
  const hide = () => setOpen(false);

  const onPointerDown = (e: PointerEvent) => {
    if (e.target instanceof Node && !root?.contains(e.target)) hide();
  };
  createEffect(open, (isOpen) => {
    if (!isOpen) return undefined;
    document.addEventListener("pointerdown", onPointerDown, true);
    return () => document.removeEventListener("pointerdown", onPointerDown, true);
  });

  return {
    open,
    show: () => {
      if (open()) return;
      setAlignsRight(false);
      setOpen(true);
    },
    hide,
    alignsRight,
    rootRef: (element) => {
      root = element;
      // Listeners on the element, not the delegated ones of Solid, so that
      // Escape stops here, before a popover around this one and the page.
      element.addEventListener("keydown", (e) => {
        if (e.key !== "Escape" || !open()) return;
        e.preventDefault();
        e.stopPropagation();
        hide();
        options.returnFocusTo().focus();
      });
      element.addEventListener("focusout", (e) => {
        if (e.relatedTarget instanceof Node && !element.contains(e.relatedTarget)) hide();
      });
    },
    // The popover has its size once it is in the page.
    panelRef: (panel) =>
      queueMicrotask(() => {
        if (panel.getBoundingClientRect().right > window.innerWidth - WINDOW_EDGE_GAP_PX) {
          setAlignsRight(true);
        }
      }),
  };
}
