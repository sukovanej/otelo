import type { JSX } from "@solidjs/web";

import Button from "./Button";
import Modal from "./Modal";

interface ConfirmDialogProps {
  readonly title: string;
  readonly message: JSX.Element;
  readonly confirmLabel: string;
  readonly busy?: boolean;
  readonly onConfirm: () => void;
  readonly onCancel: () => void;
}

export default function ConfirmDialog(props: ConfirmDialogProps) {
  return (
    <Modal label={props.title} size="dialog" onClose={props.onCancel}>
      <div class="flex flex-col gap-2 px-5 pt-4 pb-2">
        <h2 class="m-0 text-md font-semibold">{props.title}</h2>
        <div class="text-muted">{props.message}</div>
      </div>
      <div class="flex justify-end gap-2 px-5 pt-3 pb-4">
        <Button onClick={() => props.onCancel()}>Cancel</Button>
        <Button variant="danger" disabled={props.busy} onClick={() => props.onConfirm()}>
          {props.confirmLabel}
        </Button>
      </div>
    </Modal>
  );
}
