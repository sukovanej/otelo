import { A } from "@solidjs/router";
import { Show } from "solid-js";

import type { LogLine } from "@otelo/api";
import { Level } from "@otelo/ui";

import { link } from "../../classes";
import ServiceName from "../../ServiceName";
import SidePanel from "../../SidePanel";
import { formatTime, parseTime } from "../../time";
import { toLevelName } from "../level";
import LinePanelContent from "./line-panel-content";

interface LinePanelProps {
  readonly line: LogLine;
  readonly linksToTrace: boolean;
  readonly onFilter: (term: string) => void;
  readonly onClose: () => void;
}

export default function LinePanel(props: LinePanelProps) {
  return (
    <SidePanel
      label="Log line"
      onClose={props.onClose}
      header={
        <>
          <Level level={toLevelName(props.line.severity)} />
          <span class="font-mono text-sm">{formatTime(parseTime(props.line.logged_at))}</span>
          <span class="min-w-0 font-mono text-sm text-muted">
            <ServiceName name={props.line.service} resource={props.line.resource} />
          </span>
          <Show when={props.linksToTrace && props.line.trace_id}>
            {(traceId) => (
              <A href={`/traces/${traceId()}`} class={link}>
                Trace
              </A>
            )}
          </Show>
        </>
      }
    >
      <LinePanelContent line={props.line} onFilter={props.onFilter} />
    </SidePanel>
  );
}
