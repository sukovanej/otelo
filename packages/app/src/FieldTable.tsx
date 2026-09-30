import { For, Show } from "solid-js";

import type { AttributeValue } from "@otelo/api";
import { Button, SqlCode } from "@otelo/ui";

import type { Field } from "./field";

interface FieldTableProps {
  readonly fields: ReadonlyArray<Field>;
  readonly pluralNoun: string;
  readonly onFilter: (term: string) => void;
}

export default function FieldTable(props: FieldTableProps) {
  return (
    <table class="w-full border-collapse">
      <tbody>
        <For each={props.fields}>
          {(field) => (
            <tr class="group">
              <th
                scope="row"
                title={field.query.kind === "unnamed" ? undefined : field.query.name}
                class="w-[1%] py-px pr-4 text-left align-top font-normal whitespace-nowrap text-muted"
              >
                {field.label}
              </th>
              <td class="py-px pr-2 align-top whitespace-pre-wrap wrap-anywhere">
                <Show when={field.isSqlQuery} fallback={formatAttributeValue(field.value)}>
                  <SqlCode text={formatAttributeValue(field.value)} />
                </Show>
              </td>
              <td class="w-[1%] py-px align-top whitespace-nowrap">
                <Show when={field.query.kind === "comparable" && field.query}>
                  {(query) => (
                    <>
                      <Button
                        size="sm"
                        class="ml-0.5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                        title={`Keep ${props.pluralNoun} where ${query().name} = ${query().literal}`}
                        onClick={() => props.onFilter(`${query().name} = ${query().literal}`)}
                      >
                        =
                      </Button>
                      <Button
                        size="sm"
                        class="ml-0.5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                        title={`Drop ${props.pluralNoun} where ${query().name} = ${query().literal}`}
                        onClick={() => props.onFilter(`${query().name} != ${query().literal}`)}
                      >
                        ≠
                      </Button>
                    </>
                  )}
                </Show>
              </td>
            </tr>
          )}
        </For>
      </tbody>
    </table>
  );
}

function formatAttributeValue(value: AttributeValue): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}
