import type { JSX } from "@solidjs/web";
import { For, Show } from "solid-js";

import type { Attributes } from "@otelo/api";
import { DatabaseSystemIcon, toDatabaseName } from "@otelo/icons";
import { LevelBadge } from "@otelo/ui";

import HttpMethodBadge from "../../HttpMethodBadge";
import HttpRoute from "../../HttpRoute";
import HttpStatusBadge from "../../HttpStatusBadge";
import { parseLevelName } from "../../logs/level";
import { formatAttributeValue } from "../../metrics/metric";
import { isSqlSystem, readAttributeMeaning, toDatabaseIconId } from "../../semantics";
import SqlQueryStart from "../../SqlQueryStart";
import type { GroupKey } from "../measure";

interface DashboardWidgetGroupTitleProps {
  readonly label: string;
  readonly groupKey: GroupKey;
}

export default function DashboardWidgetGroupTitle(props: DashboardWidgetGroupTitleProps) {
  const hasValues = () => props.groupKey.by.some((field) => field in props.groupKey.attributes);
  return (
    <Show when={hasValues()} fallback={<span class="truncate">{props.label}</span>}>
      <span class="flex min-w-0 items-baseline gap-1.5 overflow-hidden" title={props.label}>
        <For each={props.groupKey.by} keyed={false}>
          {(field, index) => (
            <>
              <Show when={index > 0}>
                <span class="text-muted">·</span>
              </Show>
              {drawGroupValue(field(), props.groupKey.attributes)}
            </>
          )}
        </For>
      </span>
    </Show>
  );
}

function drawGroupValue(field: string, attributes: Attributes): JSX.Element {
  const value = attributes[field];
  const text = formatAttributeValue(value);
  const level = field === "level" ? parseLevelName(text) : undefined;
  if (level) return <LevelBadge level={level} />;
  const meaning = readAttributeMeaning(field);
  if (meaning === "http-method" && typeof value === "string") {
    return <HttpMethodBadge method={value.toUpperCase()} />;
  }
  const status = Number(value);
  if (meaning === "http-status" && Number.isInteger(status) && status > 0) {
    return <HttpStatusBadge status={status} />;
  }
  if (meaning === "http-route" && typeof value === "string") return <HttpRoute route={value} />;
  if (meaning === "database-system" && typeof value === "string") {
    const iconId = toDatabaseIconId(value);
    return (
      <span class="flex shrink-0 items-baseline gap-1.5 whitespace-nowrap">
        <DatabaseSystemIcon system={iconId} />
        {toDatabaseName(iconId)}
      </span>
    );
  }
  if (meaning === "database-query" && typeof value === "string" && holdsSqlQueries(attributes)) {
    return <SqlQueryStart query={value} />;
  }
  return <span class="truncate">{text}</span>;
}

function holdsSqlQueries(attributes: Attributes): boolean {
  const system = Object.entries(attributes).find(
    ([field]) => readAttributeMeaning(field) === "database-system",
  )?.[1];
  return typeof system !== "string" || isSqlSystem(system);
}
