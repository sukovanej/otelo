import { type JSX, onCleanup, onMount } from "solid-js";

/**
 * A modal over the page, which a click beside it, Escape, or the page's own
 * Close button ends with `onClose`. The keys stay in the modal, away from the
 * page under it. `onEscape` can take Escape first, such as to close a panel
 * in the modal, and returns whether it did.
 *
 * A closing dialog gives the focus back to what had it when the dialog
 * opened, such as the row a click opened it from, and the focus ring shows
 * on it when the browser takes the last input for the keyboard's. The
 * modal keeps that for a modal opened with the keyboard, which is where its
 * user goes on from, and takes the focus off the row for one opened with the
 * pointer, which needs no ring.
 */
export default function Modal(props: {
  label: string;
  onClose: () => void;
  onEscape?: () => boolean;
  children: JSX.Element;
}) {
  let dialog!: HTMLDialogElement;
  let opener: HTMLElement | undefined;
  let byKeyboard = false;
  let removing = false;

  onMount(() => {
    const active = document.activeElement;
    if (active instanceof HTMLElement) {
      opener = active;
      byKeyboard = active.matches(":focus-visible");
    }
    dialog.showModal();
  });

  onCleanup(() => {
    removing = true;
    // Closing the dialog, not only removing it, is what gives the focus back.
    if (dialog.open) dialog.close();
    if (!opener?.isConnected) return;
    if (byKeyboard) opener.focus();
    else if (document.activeElement === opener) opener.blur();
  });

  return (
    <dialog
      ref={dialog}
      aria-label={props.label}
      class="m-auto h-[88dvh] max-h-none w-[min(94vw,96rem)] max-w-none overflow-hidden rounded-lg border border-line bg-surface p-0 text-ink shadow-popup backdrop:bg-[rgb(0_0_0/0.45)] open:flex open:flex-col"
      // The dialog gets a click on its backdrop; its content covers the rest.
      onClick={(e) => {
        if (e.target === dialog) props.onClose();
      }}
      on:keydown={(e) => {
        if (e.key === "/") e.stopPropagation();
        if (e.key !== "Escape") return;
        // The page removes the modal after `onClose`, which closes the
        // dialog, so every way out takes the same path.
        e.preventDefault();
        e.stopPropagation();
        if (!props.onEscape?.()) props.onClose();
      }}
      onClose={() => {
        if (!removing) props.onClose();
      }}
    >
      {props.children}
    </dialog>
  );
}
