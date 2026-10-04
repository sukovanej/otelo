import type { JSX } from "@solidjs/web";
import { onCleanup, onSettled } from "solid-js";

const SIZE_CLASSES: Record<ModalSize, string> = {
  page: "h-[88dvh] w-[min(94vw,96rem)]",
  dialog: "w-[min(92vw,28rem)]",
};

type ModalSize = "page" | "dialog";

interface ModalProps {
  readonly label: string;
  readonly size?: ModalSize;
  readonly onClose: () => void;
  readonly onEscape?: () => boolean;
  readonly children: JSX.Element;
}

export default function Modal(props: ModalProps) {
  let dialog!: HTMLDialogElement;
  let opener: HTMLElement | undefined;
  let openedByKeyboard = false;
  let removing = false;

  onSettled(() => {
    const active = document.activeElement;
    if (active instanceof HTMLElement) {
      opener = active;
      openedByKeyboard = active.matches(":focus-visible");
    }
    dialog.showModal();
  });

  onCleanup(() => {
    removing = true;
    // Moving the focus runs blur handlers that write signals, which must wait
    // until the disposal is over.
    queueMicrotask(() => {
      if (dialog.open) dialog.close();
      if (!opener?.isConnected) return;
      // After a modal opened with the keyboard the opener keeps the focus, to go
      // on from. After one opened with the pointer it shows no focus ring.
      if (openedByKeyboard) opener.focus();
      else if (document.activeElement === opener) opener.blur();
    });
  });

  return (
    <dialog
      ref={dialog}
      aria-label={props.label}
      class={`m-auto max-h-none ${SIZE_CLASSES[props.size ?? "page"]} max-w-none overflow-hidden rounded-lg border border-line bg-surface p-0 text-ink shadow-popup backdrop:bg-[rgb(0_0_0/0.45)] open:flex open:flex-col`}
      // The dialog gets a click on its backdrop; its content covers the rest.
      onClick={(e) => {
        if (e.target === dialog) props.onClose();
      }}
      onKeyDown={(e) => {
        // "/" would focus the query of the page under the modal.
        if (e.key === "/") e.stopPropagation();
        if (e.key !== "Escape" || e.defaultPrevented) return;
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
