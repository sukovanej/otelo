import { A } from "@solidjs/router";
import { createSignal } from "solid-js";

import { link } from "../classes";
import Modal from "../Modal";
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
  span?: string | undefined;
  onFilterSpans: (term: string) => void;
  onFilterLogs: (term: string) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = createSignal<TraceTab>("spans");
  // The modal reads the first span once; it keeps its selection after that.
  const [span, setSpan] = createSignal(props.span);
  const state = createTraceState(tab, setTab, span, setSpan);
  const page = () => tracePath(props.id, state);

  return (
    <Modal label="Trace" onClose={props.onClose} onEscape={() => closePanel(state)}>
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
    </Modal>
  );
}
