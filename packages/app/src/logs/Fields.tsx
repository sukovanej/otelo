import { Button } from "@siner/ui";
import { For, Show } from "solid-js";
import type { Json, LogLine } from "../api";
import { attributeField, literal, quote, resourceField } from "../query";
import { formatDateTime, parseTime } from "../time";
import { body, detail, heading, times } from "./classes";

interface Field {
  /** The name a query uses for the field, or `undefined` when it has none. */
  name: string | undefined;
  label: string;
  value: Json;
  /** The literal a comparison uses, or `undefined` when none matches. */
  literal: string | undefined;
}

const sorted = (record: Record<string, Json>) =>
  Object.entries(record).toSorted(([a], [b]) => a.localeCompare(b));

/** A built-in field of a line, compared as a string unless `lit` says how. */
const builtin = (name: string, value: string | null, lit = value && quote(value)): Field => ({
  name,
  label: name,
  value,
  literal: lit ?? undefined,
});

function fields(line: LogLine): { title: string; fields: Field[] }[] {
  return [
    {
      title: "Line",
      fields: [
        builtin("service", line.service),
        builtin("level", line.level, line.level.toLowerCase()),
        builtin("source", line.source),
        builtin("trace_id", line.trace_id),
        builtin("span_id", line.span_id),
      ].filter((field) => field.value !== null),
    },
    {
      title: "Attributes",
      fields: sorted(line.attributes).map(([key, value]) => ({
        name: attributeField(key),
        label: key,
        value,
        literal: literal(value),
      })),
    },
    {
      title: "Resource",
      fields: sorted(line.resource).map(([key, value]) => ({
        name: resourceField(key),
        label: key,
        value,
        literal: literal(value),
      })),
    },
  ].filter((section) => section.fields.length > 0);
}

const show = (value: Json) => (typeof value === "string" ? value : JSON.stringify(value));

/** The whole of one log line, with buttons that add a field to the query. */
export default function Fields(props: { line: LogLine; onFilter: (term: string) => void }) {
  return (
    <div class={detail}>
      <pre class={body}>{props.line.body}</pre>
      <div class={times}>
        {formatDateTime(parseTime(props.line.time))} local · {props.line.time}
      </div>
      <For each={fields(props.line)}>
        {(section) => (
          <section>
            <h3 class={heading}>{section.title}</h3>
            <table class="w-full border-collapse">
              <tbody>
                <For each={section.fields}>
                  {(field) => (
                    <tr class="group">
                      <th
                        scope="row"
                        title={field.name}
                        class="w-[1%] min-w-[20ch] py-px pr-2 text-left align-top font-normal whitespace-nowrap text-muted"
                      >
                        {field.label}
                      </th>
                      <td class="py-px pr-2 align-top whitespace-pre-wrap wrap-anywhere">
                        {show(field.value)}
                      </td>
                      <td class="w-[1%] py-px align-top whitespace-nowrap">
                        <Show when={field.name && field.literal}>
                          <Button
                            size="sm"
                            class="ml-0.5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                            title={`Keep lines where ${field.name} = ${field.literal}`}
                            onClick={() => props.onFilter(`${field.name} = ${field.literal}`)}
                          >
                            =
                          </Button>
                          <Button
                            size="sm"
                            class="ml-0.5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                            title={`Drop lines where ${field.name} = ${field.literal}`}
                            onClick={() => props.onFilter(`${field.name} != ${field.literal}`)}
                          >
                            ≠
                          </Button>
                        </Show>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </section>
        )}
      </For>
    </div>
  );
}
