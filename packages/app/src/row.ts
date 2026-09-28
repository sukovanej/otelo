/** Whether a click on a link is a plain left click, which the page can take
 * instead of the browser. A click with a modifier key opens the address the
 * browser's way, such as in a new tab. */
export const plainClick = (e: MouseEvent) =>
  e.button === 0 && !e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey;
