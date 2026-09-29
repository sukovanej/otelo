import { Match, Switch } from "solid-js";

import type { TargetKey } from "@otelo/api";
import { DatabaseSystemIcon, GlobeIcon, SpanIcon } from "@otelo/icons";
import { Tooltip } from "@otelo/ui";

import { databaseId } from "../semantics";
import { targetSystem } from "./target";

/** What a call goes to, after an icon of what it is: the icon of the
 * database system, a globe for a host, or a plain span. A target with a
 * system and a name shows the system faint when the icon does not say it.
 * The name truncates in a narrow cell. */
export default function TargetName(props: { target: TargetKey }) {
  const system = () => targetSystem(props.target);
  const title = () => `${system()}${props.target.name === null ? "" : ` ${props.target.name}`}`;
  return (
    <span class="flex min-w-0 items-baseline gap-1.5" title={title()}>
      <Tooltip content={system()} class="self-center">
        <Switch fallback={<SpanIcon class="text-muted" title={system()} />}>
          <Match when={props.target.type === "database" && props.target.system}>
            {(id) => <DatabaseSystemIcon system={databaseId(id())} title={system()} />}
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
