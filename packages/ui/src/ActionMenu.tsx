import type { JSX } from "@solidjs/web";
import { createSignal, createUniqueId, For } from "solid-js";

import { MoreIcon } from "@otelo/icons";

import Button from "./Button";
import { cx, option } from "./classes";
import { moveListIndex, toListStep } from "./keys";

const MENU_GAP_PX = 4;

const WINDOW_EDGE_GAP_PX = 8;

const ITEM_TONE_CLASSES: Record<ActionMenuItemTone, string> = {
  default: "text-ink",
  danger: "text-error",
};

export interface ActionMenuItem {
  readonly label: string;
  readonly icon: () => JSX.Element;
  readonly tone: ActionMenuItemTone;
  readonly onSelect: () => void;
}

type ActionMenuItemTone = "default" | "danger";

interface ActionMenuProps {
  readonly label: string;
  readonly items: ReadonlyArray<ActionMenuItem>;
}

// The menu is a popover in the top layer, so a panel that clips what overflows
// it does not cut the menu off.
export default function ActionMenu(props: ActionMenuProps) {
  let trigger!: HTMLButtonElement;
  let menu!: HTMLDivElement;
  const menuId = createUniqueId();
  const [open, setOpen] = createSignal(false);
  const itemElements = () => [...menu.querySelectorAll<HTMLElement>("[role=menuitem]")];
  const closeMenu = () => menu.hidePopover();

  const placeMenu = () => {
    const anchor = trigger.getBoundingClientRect();
    const width = menu.offsetWidth;
    const left = Math.min(anchor.right - width, window.innerWidth - width - WINDOW_EDGE_GAP_PX);
    menu.style.top = `${anchor.bottom + MENU_GAP_PX}px`;
    menu.style.left = `${Math.max(WINDOW_EDGE_GAP_PX, left)}px`;
  };
  const onToggle = (e: ToggleEvent) => {
    const isOpen = e.newState === "open";
    setOpen(isOpen);
    if (isOpen) {
      placeMenu();
      itemElements()[0]?.focus();
      window.addEventListener("scroll", closeMenu, true);
      window.addEventListener("resize", closeMenu);
    } else {
      window.removeEventListener("scroll", closeMenu, true);
      window.removeEventListener("resize", closeMenu);
    }
  };
  const onMenuKeyDown = (e: KeyboardEvent) => {
    const elements = itemElements();
    const focusedIndex = elements.findIndex((element) => element === document.activeElement);
    const step = toListStep(e);
    if (step !== 0) {
      e.preventDefault();
      elements[moveListIndex(focusedIndex, step, elements.length)]?.focus();
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      elements[e.key === "Home" ? 0 : elements.length - 1]?.focus();
    }
  };

  return (
    <>
      <Button
        ref={trigger}
        size="sm"
        variant="ghost"
        aria-label={props.label}
        title={props.label}
        aria-haspopup="menu"
        aria-expanded={open() ? "true" : "false"}
        popovertarget={menuId}
      >
        <MoreIcon size={14} />
      </Button>
      <div
        ref={menu}
        id={menuId}
        popover="auto"
        role="menu"
        aria-label={props.label}
        class="inset-auto m-0 min-w-40 rounded-lg border border-line bg-surface p-1 text-ink shadow-popup"
        onToggle={onToggle}
        onKeyDown={onMenuKeyDown}
      >
        <For each={props.items}>
          {(item) => (
            <button
              type="button"
              role="menuitem"
              tabindex={-1}
              class={cx(
                option,
                "w-full items-center gap-2 border-0 bg-transparent text-left outline-none hover:bg-active focus:bg-active",
                ITEM_TONE_CLASSES[item.tone],
              )}
              onClick={() => {
                closeMenu();
                item.onSelect();
              }}
            >
              {item.icon()}
              {item.label}
            </button>
          )}
        </For>
      </div>
    </>
  );
}
