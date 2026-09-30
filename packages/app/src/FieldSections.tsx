import { For } from "solid-js";

import { sectionHeading } from "./classes";
import type { FieldSection } from "./field";
import FieldTable from "./FieldTable";

interface FieldSectionsProps {
  readonly sections: ReadonlyArray<FieldSection>;
  readonly pluralNoun: string;
  readonly onFilter: (term: string) => void;
}

export default function FieldSections(props: FieldSectionsProps) {
  return (
    <For each={props.sections.filter((section) => section.fields.length > 0)}>
      {(section) => (
        <section>
          <h3 class={sectionHeading}>{section.title}</h3>
          <FieldTable
            fields={section.fields}
            pluralNoun={props.pluralNoun}
            onFilter={props.onFilter}
          />
        </section>
      )}
    </For>
  );
}
