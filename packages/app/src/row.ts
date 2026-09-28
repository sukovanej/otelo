import type { JSX } from "solid-js";

/** The attributes of a list row that toggles on a click, or on Enter or
 * Space. A click that ends a text selection is not a toggle. */
export const toggleRow = (
  toggle: () => void,
): Pick<JSX.HTMLAttributes<HTMLDivElement>, "role" | "tabIndex" | "onClick" | "onKeyDown"> => ({
  role: "button",
  tabIndex: 0,
  onClick: () => {
    if (window.getSelection()?.isCollapsed !== false) toggle();
  },
  onKeyDown: (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      toggle();
    }
  },
});

/** Whether a click on a link is a plain left click, which the page can take
 * instead of the browser. A click with a modifier key opens the address the
 * browser's way, such as in a new tab. */
export const plainClick = (e: MouseEvent) =>
  e.button === 0 && !e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey;
