import { A } from "@solidjs/router";
import { createSignal } from "solid-js";

import { CloseButton, Modal } from "@otelo/ui";

import { link } from "../../classes";
import TraceView, { closePanel, createTraceState, toTracePath, type TraceTab } from "../TraceView";

interface TracesPageTraceModalProps {
  readonly id: string;
  readonly initialSpanId: string | undefined;
  readonly onFilterSpans: (term: string) => void;
  readonly onFilterLogs: (term: string) => void;
  readonly onClose: () => void;
}

export default function TracesPageTraceModal(props: TracesPageTraceModalProps) {
  const [tab, setTab] = createSignal<TraceTab>("spans");
  // The modal reads the first span once; it keeps its selection after that.
  const [spanId, setSpanId] = createSignal(props.initialSpanId);
  const state = createTraceState(tab, setTab, spanId, setSpanId);
  const pagePath = () => toTracePath(props.id, state);

  return (
    <Modal label="Trace" onClose={props.onClose} onEscape={() => closePanel(state)}>
      <TraceView
        id={props.id}
        state={state}
        onFilterSpans={props.onFilterSpans}
        onFilterLogs={props.onFilterLogs}
        actions={
          <>
            <A href={pagePath()} class={link} title="The page of this trace, to share">
              Open page
            </A>
            <CloseButton onClose={props.onClose} />
          </>
        }
      />
    </Modal>
  );
}
