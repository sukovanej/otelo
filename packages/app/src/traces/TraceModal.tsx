import { A } from "@solidjs/router";
import { createSignal, onMount } from "solid-js";
import { link } from "../classes";
import { CloseButton } from "../Panel";
import TraceView, { closePanel, createTraceState, type TraceTab, tracePath } from "./TraceView";

/**
 * One trace over the list it was opened from. Escape closes the panel in
 * it first, then the modal. The link to the page of the trace keeps the tab
 * and the span it shows, so it is the one to share.
 */
export default function TraceModal(props: {
  id: string;
  /** The span to select first. */
  span?: string;
  onFilterSpans: (term: string) => void;
  onFilterLogs: (term: string) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = createSignal<TraceTab>("spans");
  // The modal reads the first span once; it keeps its selection after that.
  const [span, setSpan] = createSignal(props.span);
  const state = createTraceState(tab, setTab, span, setSpan);
  const page = () => tracePath(props.id, state);

  let dialog!: HTMLDialogElement;
  onMount(() => dialog.showModal());

  return (
    <dialog
      ref={dialog}
      aria-label="Trace"
      class="m-auto h-[88dvh] max-h-none w-[min(94vw,96rem)] max-w-none overflow-hidden rounded-lg border border-line bg-surface p-0 text-ink shadow-popup backdrop:bg-[rgb(0_0_0/0.45)] open:flex open:flex-col"
      // The dialog gets a click on its backdrop; its content covers the rest.
      onClick={(e) => {
        if (e.target === dialog) props.onClose();
      }}
      on:keydown={(e) => {
        // The keys stay in the modal, away from the page under it.
        if (e.key === "/") e.stopPropagation();
        if (e.key !== "Escape") return;
        e.preventDefault();
        e.stopPropagation();
        if (!closePanel(state)) props.onClose();
      }}
      onClose={() => props.onClose()}
    >
      <TraceView
        id={props.id}
        state={state}
        onFilterSpans={props.onFilterSpans}
        onFilterLogs={props.onFilterLogs}
        actions={
          <>
            <A href={page()} class={link} title="The page of this trace, to share">
              Open page
            </A>
            <CloseButton onClose={props.onClose} />
          </>
        }
      />
    </dialog>
  );
}
