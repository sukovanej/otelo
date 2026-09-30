import { Index } from "solid-js";

import { cx } from "../classes";
import { type Piece, pickPieceClass } from "../highlight";

interface QueryInputColorsProps {
  readonly class: string;
  readonly pieces: ReadonlyArray<Piece>;
  readonly scrollLeft: number;
  readonly ref: (pieceElements: HTMLDivElement) => void;
}

export default function QueryInputColors(props: QueryInputColorsProps) {
  return (
    <div
      aria-hidden="true"
      class={cx(props.class, "pointer-events-none absolute inset-0 border-transparent")}
    >
      {/* Cuts the text at the padding, as the input does. */}
      <div class="flex h-full items-center overflow-hidden">
        <div
          ref={props.ref}
          class="whitespace-pre"
          style={{ transform: `translateX(${-props.scrollLeft}px)` }}
        >
          <Index each={props.pieces}>
            {(piece) => <span class={pickPieceClass(piece())}>{piece().text}</span>}
          </Index>
        </div>
      </div>
    </div>
  );
}
