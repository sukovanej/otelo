import { A } from "@solidjs/router";
import { Show } from "solid-js";

import type { LogLine } from "@siner/api";
import { Level } from "@siner/ui";

import { link } from "../classes";
import Panel from "../Panel";
import Service from "../Service";
import { formatTime, parseTime } from "../time";
import Fields from "./Fields";

/** One log line in full, in a panel beside the list, with a link to its
 * trace unless `inTrace` says the page shows it. */
export default function LinePanel(props: {
  line: LogLine;
  inTrace?: boolean;
  onFilter: (term: string) => void;
  onClose: () => void;
}) {
  return (
    <Panel
      label="Log line"
      onClose={props.onClose}
      header={
        <>
          <Level level={props.line.level} />
          <span class="font-mono text-sm">{formatTime(parseTime(props.line.time))}</span>
          <span class="min-w-0 font-mono text-sm text-muted">
            <Service name={props.line.service} resource={props.line.resource} />
          </span>
          <Show when={!props.inTrace && props.line.trace_id}>
            {(id) => (
              <A href={`/traces/${id()}`} class={link}>
                Trace
              </A>
            )}
          </Show>
        </>
      }
    >
      <Fields line={props.line} onFilter={props.onFilter} />
    </Panel>
  );
}
