import { Match, Switch } from "solid-js";

import type { TargetKey } from "@otelo/api";
import { DatabaseSystemIcon, GlobeIcon, SpanIcon } from "@otelo/icons";
import { Tooltip } from "@otelo/ui";

import { toDatabaseIconId } from "../../semantics";
import { nameTargetSystem } from "../target";

interface ServicePageTargetNameProps {
  readonly target: TargetKey;
}

export default function ServicePageTargetName(props: ServicePageTargetNameProps) {
  const system = () => nameTargetSystem(props.target);
  const title = () => `${system()}${props.target.name === null ? "" : ` ${props.target.name}`}`;
  return (
    <span class="flex min-w-0 items-baseline gap-1.5" title={title()}>
      <Tooltip content={system()} class="self-center">
        <Switch fallback={<SpanIcon class="text-muted" title={system()} />}>
          <Match when={props.target.type === "database" && props.target.system}>
            {(databaseSystem) => (
              <DatabaseSystemIcon system={toDatabaseIconId(databaseSystem())} title={system()} />
            )}
          </Match>
          <Match when={props.target.type === "http"}>
            <GlobeIcon class="text-muted" title={system()} />
          </Match>
        </Switch>
      </Tooltip>
      <Switch>
        <Match when={props.target.name === null}>
          <span class="truncate text-muted">{system()}</span>
        </Match>
        <Match when={props.target.type === "rpc" || props.target.type === "messaging"}>
          <span class="text-muted">{system()}</span>
          <span class="truncate">{props.target.name}</span>
        </Match>
        <Match when={true}>
          <span class="truncate">{props.target.name}</span>
        </Match>
      </Switch>
    </span>
  );
}
