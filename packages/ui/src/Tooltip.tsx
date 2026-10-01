import { type JSX, Portal } from "@solidjs/web";
import { createEffect, createSignal, createUniqueId, Show } from "solid-js";

const POINTER_OFFSET_PX = 18;

const TIP_GAP_PX = 6;

const PLAIN_TIP_CLASSES = "bg-ink px-2 py-1 font-sans whitespace-nowrap text-surface";

const RICH_TIP_CLASSES =
  "max-h-[80vh] max-w-[min(44rem,90vw)] overflow-hidden border border-line bg-surface px-2.5 py-2 text-ink";

interface TipAnchor {
  readonly centerX: number;
  readonly topWhenBelow: number;
  readonly bottomWhenAbove: number;
}

interface TooltipProps {
  readonly content: JSX.Element;
  readonly rich?: boolean;
  readonly class?: string;
  readonly children: JSX.Element;
}

export default function Tooltip(props: TooltipProps) {
  const tipId = createUniqueId();
  const [anchor, setAnchor] = createSignal<TipAnchor>();
  const [tip, setTip] = createSignal<HTMLDivElement>();
  let trigger!: HTMLSpanElement;

  const anchorAtPointer = (e: PointerEvent) =>
    setAnchor({
      centerX: e.clientX,
      topWhenBelow: e.clientY + POINTER_OFFSET_PX,
      bottomWhenAbove: e.clientY - TIP_GAP_PX,
    });
  const anchorUnderTrigger = () => {
    const box = trigger.getBoundingClientRect();
    setAnchor({
      centerX: box.left + box.width / 2,
      topWhenBelow: box.bottom + TIP_GAP_PX,
      bottomWhenAbove: box.top - TIP_GAP_PX,
    });
  };
  const hideTip = () => setAnchor(undefined);

  // Places the tip once it is in the page, since that needs its size.
  createEffect(
    () => [anchor(), tip()] as const,
    ([shownAnchor, shownTip]) => {
      if (!shownAnchor || !shownTip) return;
      const { offsetWidth: width, offsetHeight: height } = shownTip;
      const left = Math.min(
        Math.max(shownAnchor.centerX - width / 2, TIP_GAP_PX),
        window.innerWidth - width - TIP_GAP_PX,
      );
      const top =
        shownAnchor.topWhenBelow + height + TIP_GAP_PX <= window.innerHeight
          ? shownAnchor.topWhenBelow
          : shownAnchor.bottomWhenAbove - height;
      shownTip.style.left = `${left}px`;
      shownTip.style.top = `${Math.max(top, TIP_GAP_PX)}px`;
    },
  );

  return (
    <>
      <span
        ref={trigger}
        class={`inline-flex ${props.class ?? ""}`}
        aria-describedby={anchor() ? tipId : undefined}
        onPointerEnter={anchorAtPointer}
        onPointerMove={anchorAtPointer}
        onPointerLeave={hideTip}
        onFocusIn={anchorUnderTrigger}
        onFocusOut={hideTip}
      >
        {props.children}
      </span>
      <Show when={anchor()}>
        <Portal
          // In the open dialog around the trigger, or else in the body, so no
          // panel clips the tip.
          mount={trigger.closest("dialog[open]") ?? document.body}
        >
          <div
            ref={setTip}
            id={tipId}
            role="tooltip"
            class={`pointer-events-none fixed top-0 left-0 z-50 rounded-md text-xs shadow-popup ${props.rich ? RICH_TIP_CLASSES : PLAIN_TIP_CLASSES}`}
          >
            {props.content}
          </div>
        </Portal>
      </Show>
    </>
  );
}
