import { CloseIcon } from "@otelo/icons";
import { Button } from "@otelo/ui";

interface CloseButtonProps {
  readonly onClose: () => void;
}

export default function CloseButton(props: CloseButtonProps) {
  return (
    <Button
      size="sm"
      variant="ghost"
      aria-label="Close"
      title="Close (Esc)"
      onClick={() => props.onClose()}
    >
      <CloseIcon size={13} />
    </Button>
  );
}
