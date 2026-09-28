import type { LogLine } from "../api";
import { body, times } from "../classes";
import FieldTable, { attributes, builtin, resource } from "../FieldTable";
import { formatDateTime, parseTime } from "../time";

/** The whole of one log line, with buttons that add a field to the query. */
export default function Fields(props: { line: LogLine; onFilter: (term: string) => void }) {
  const sections = () => [
    {
      title: "Line",
      fields: [
        builtin("service", props.line.service),
        builtin("level", props.line.level, props.line.level.toLowerCase()),
        builtin("source", props.line.source),
        builtin("trace_id", props.line.trace_id),
        builtin("span_id", props.line.span_id),
      ].filter((field) => field.value !== null),
    },
    { title: "Attributes", fields: attributes(props.line.attributes) },
    { title: "Resource", fields: resource(props.line.resource) },
  ];
  return (
    <div class="font-mono">
      <pre class={body}>{props.line.body}</pre>
      <div class={times}>
        {formatDateTime(parseTime(props.line.time))} local · {props.line.time}
      </div>
      <FieldTable sections={sections()} noun="lines" onFilter={props.onFilter} />
    </div>
  );
}
