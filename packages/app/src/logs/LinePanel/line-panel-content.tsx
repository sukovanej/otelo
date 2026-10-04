import type { LogLine } from "@otelo/api";

import { logBody, timesLine } from "../../classes";
import {
  type FieldSection,
  linkField,
  listAttributeFields,
  listResourceFields,
  toBuiltinField,
} from "../../field";
import FieldSections from "../../FieldSections";
import { toServicePagePath, toTracePagePath } from "../../path";
import { formatDateTime, parseTime } from "../../time";
import { toLevelName } from "../level";

interface LinePanelContentProps {
  readonly line: LogLine;
  readonly onFilter: (term: string) => void;
}

export default function LinePanelContent(props: LinePanelContentProps) {
  const level = () => toLevelName(props.line.severity);
  const sections = (): FieldSection[] => [
    {
      title: "Line",
      fields: [
        linkField(
          toBuiltinField("service", props.line.service),
          toServicePagePath(props.line.service),
        ),
        toBuiltinField("level", level(), level().toLowerCase()),
        linkField(
          toBuiltinField("trace_id", props.line.trace_id),
          props.line.trace_id && toTracePagePath(props.line.trace_id),
        ),
        linkField(
          toBuiltinField("span_id", props.line.span_id),
          props.line.trace_id &&
            props.line.span_id &&
            toTracePagePath(props.line.trace_id, props.line.span_id),
        ),
      ].filter((field) => field.value !== null),
    },
    { title: "Attributes", fields: listAttributeFields(props.line.attributes) },
    { title: "Resource", fields: listResourceFields(props.line.resource) },
  ];
  return (
    <div class="font-mono">
      <pre class={logBody}>{props.line.body}</pre>
      <div class={timesLine}>
        {formatDateTime(parseTime(props.line.logged_at))} local · {props.line.logged_at}
      </div>
      <FieldSections sections={sections()} pluralNoun="lines" onFilter={props.onFilter} />
    </div>
  );
}
