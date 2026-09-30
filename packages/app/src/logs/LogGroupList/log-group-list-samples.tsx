import { For, Show } from "solid-js";

import type { LogGroup } from "@otelo/api";
import { Button } from "@otelo/ui";

import { logBody, rowDetail, sectionHeading, timesLine } from "../../classes";
import { writeTemplateTerm } from "../../query";
import { formatTime, parseTime } from "../../time";

interface LogGroupListSamplesProps {
  readonly group: LogGroup;
  readonly onShowLines: (term: string) => void;
}

export default function LogGroupListSamples(props: LogGroupListSamplesProps) {
  const templateTerm = () => writeTemplateTerm(props.group.template);
  return (
    <div class={rowDetail}>
      <div class={timesLine}>
        From {formatTime(parseTime(props.group.first_at))} to{" "}
        {formatTime(parseTime(props.group.last_at))}
      </div>
      <h3 class={sectionHeading}>Samples</h3>
      <For each={props.group.samples}>{(sample) => <pre class={logBody}>{sample}</pre>}</For>
      <Show when={templateTerm()}>
        {(term) => (
          <Button
            class="mt-1.5"
            title={`Add ${term()} to the query`}
            onClick={() => props.onShowLines(term())}
          >
            Show lines
          </Button>
        )}
      </Show>
    </div>
  );
}
