import { Button } from "@siner/ui";
import { For, Show } from "solid-js";
import type { Json } from "./api";
import { heading } from "./classes";
import { attributeField, literal, quote, resourceField } from "./query";

export interface Field {
  /** The name a query uses for the field, or `undefined` when it has none. */
  name: string | undefined;
  label: string;
  value: Json;
  /** The literal a comparison uses, or `undefined` when none matches. */
  literal: string | undefined;
}

export interface Section {
  title: string;
  fields: Field[];
}

/** A built-in field, compared as a string unless `lit` says how. */
export const builtin = (
  name: string,
  value: string | null,
  lit = value && quote(value),
): Field => ({
  name,
  label: name,
  value,
  literal: lit ?? undefined,
});

const sorted = (record: Record<string, Json>) =>
  Object.entries(record).toSorted(([a], [b]) => a.localeCompare(b));

/** The attributes of a record, by key. */
export const attributes = (record: Record<string, Json>): Field[] =>
  sorted(record).map(([key, value]) => ({
    name: attributeField(key),
    label: key,
    value,
    literal: literal(value),
  }));

/** The attributes of a resource, by key. */
export const resource = (record: Record<string, Json>): Field[] =>
  sorted(record).map(([key, value]) => ({
    name: resourceField(key),
    label: key,
    value,
    literal: literal(value),
  }));

const show = (value: Json) => (typeof value === "string" ? value : JSON.stringify(value));

/** Fields in a table, with buttons that add a field that has a name and a
 * literal to the query. `noun` names the records, such as `lines`. */
export function Fields(props: { fields: Field[]; noun: string; onFilter: (term: string) => void }) {
  return (
    <table class="w-full border-collapse">
      <tbody>
        <For each={props.fields}>
          {(field) => (
            <tr class="group">
              <th
                scope="row"
                title={field.name}
                class="w-[1%] py-px pr-4 text-left align-top font-normal whitespace-nowrap text-muted"
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
                    title={`Keep ${props.noun} where ${field.name} = ${field.literal}`}
                    onClick={() => props.onFilter(`${field.name} = ${field.literal}`)}
                  >
                    =
                  </Button>
                  <Button
                    size="sm"
                    class="ml-0.5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                    title={`Drop ${props.noun} where ${field.name} = ${field.literal}`}
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
  );
}

/** Sections of fields under their titles, leaving out the empty ones. */
export default function FieldTable(props: {
  sections: Section[];
  noun: string;
  onFilter: (term: string) => void;
}) {
  return (
    <For each={props.sections.filter((section) => section.fields.length > 0)}>
      {(section) => (
        <section>
          <h3 class={heading}>{section.title}</h3>
          <Fields fields={section.fields} noun={props.noun} onFilter={props.onFilter} />
        </section>
      )}
    </For>
  );
}
