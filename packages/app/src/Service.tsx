import type { Attributes } from "@otelo/api";
import { LanguageIcon, languageName } from "@otelo/icons";
import { Tooltip } from "@otelo/ui";

import { language } from "./semantics";

/** The name of a service after the icon of its language, or of code when
 * its language is unknown, so the names of a list line up. The icon tells
 * the name of the language on hover. The name truncates in a narrow cell. */
export default function Service(props: { name: string; resource: Attributes }) {
  return (
    <span class="flex min-w-0 items-baseline gap-1.5">
      <Tooltip content={languageName(language(props.resource))} class="self-center">
        <LanguageIcon language={language(props.resource)} />
      </Tooltip>
      <span class="truncate">{props.name}</span>
    </span>
  );
}
