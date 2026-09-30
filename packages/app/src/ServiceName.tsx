import type { Attributes } from "@otelo/api";
import { LanguageIcon, toLanguageName } from "@otelo/icons";
import { Tooltip } from "@otelo/ui";

import { readLanguageIconId } from "./semantics";

interface ServiceNameProps {
  readonly name: string;
  readonly resource: Attributes;
}

export default function ServiceName(props: ServiceNameProps) {
  return (
    <span class="flex min-w-0 items-baseline gap-1.5">
      <Tooltip content={toLanguageName(readLanguageIconId(props.resource))} class="self-center">
        <LanguageIcon language={readLanguageIconId(props.resource)} />
      </Tooltip>
      <span class="truncate">{props.name}</span>
    </span>
  );
}
