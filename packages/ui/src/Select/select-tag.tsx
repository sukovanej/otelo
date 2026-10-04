import { CloseIcon } from "@otelo/icons";

interface SelectTagProps {
  readonly label: string;
  readonly value: string;
  readonly position: number;
  readonly count: number;
  readonly dragged: boolean;
  readonly onMove: (step: -1 | 1) => void;
  readonly onRemove: () => void;
}

export default function SelectTag(props: SelectTagProps) {
  return (
    <span
      data-reorder-value={props.value}
      tabindex={0}
      aria-label={`${props.label}, ${props.position} of ${props.count}: drag it, or press Alt with the arrow keys, to move it`}
      class={[
        "inline-flex h-5.5 max-w-full min-w-0 cursor-grab touch-none items-center gap-0.5 rounded pr-0.5 pl-1.5 outline-none select-none focus-visible:ring-2 focus-visible:ring-line-focus",
        props.dragged
          ? "border border-dashed border-accent bg-accent/5 *:invisible"
          : "bg-active hover:bg-line",
      ]}
      onKeyDown={(e) => {
        if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
          e.preventDefault();
          e.stopPropagation();
          props.onMove(e.key === "ArrowLeft" ? -1 : 1);
        } else if (e.key === "Backspace" || e.key === "Delete") {
          e.preventDefault();
          e.stopPropagation();
          props.onRemove();
        }
      }}
    >
      <span class="-translate-y-px truncate">{props.label}</span>
      <button
        type="button"
        tabindex={-1}
        data-reorder-ignore
        aria-label={`Remove ${props.label}`}
        class="flex size-4 shrink-0 translate-y-px cursor-pointer items-center justify-center rounded-sm text-muted hover:bg-hover hover:text-ink"
        onClick={(e) => {
          e.stopPropagation();
          props.onRemove();
        }}
      >
        <CloseIcon size={10} />
      </button>
    </span>
  );
}
