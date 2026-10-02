import { createSignal } from "solid-js";

import type { LogGroup } from "@otelo/api";
import { LevelBadge } from "@otelo/ui";
import { type Column, Table } from "@otelo/viz";

import { formatAge, parseTime } from "../../time";
import { toLevelName } from "../level";
import LogGroupListSamples from "./log-group-list-samples";
import LogGroupListTemplate from "./log-group-list-template";

interface LogGroupListProps {
  readonly groups: ReadonlyArray<LogGroup>;
  readonly onShowLines: (term: string) => void;
}

export default function LogGroupList(props: LogGroupListProps) {
  const [openTemplate, setOpenTemplate] = createSignal<string>();
  const columns: Column<LogGroup>[] = [
    {
      kind: "meter",
      id: "count",
      label: "Lines",
      unit: "count",
      width: "16ch",
      value: (group) => group.count,
    },
    {
      kind: "cell",
      id: "level",
      label: "Level",
      width: "max-content",
      cell: (group) => <LevelBadge level={toLevelName(group.severity)} />,
    },
    {
      kind: "cell",
      id: "template",
      label: "Template",
      width: "minmax(0,1fr)",
      cell: (group) => (
        <span class="wrap-anywhere">
          <LogGroupListTemplate text={group.template} />
        </span>
      ),
    },
    {
      kind: "text",
      id: "services",
      label: "Services",
      width: "minmax(8ch,20ch)",
      value: (group) => group.services.join(", "),
      tone: () => "muted",
    },
    {
      kind: "cell",
      id: "last",
      label: "Last",
      align: "end",
      width: "max-content",
      tone: () => "muted",
      cell: (group) => (
        <time class="whitespace-nowrap" datetime={group.last_at} title={group.last_at}>
          {formatAge(parseTime(group.last_at))}
        </time>
      ),
    },
  ];

  return (
    <Table
      label="Log templates"
      rows={props.groups}
      rowKey={(group) => group.template}
      columns={columns}
      selectedKey={openTemplate}
      onRowClick={(group) =>
        setOpenTemplate(openTemplate() === group.template ? undefined : group.template)
      }
      detail={(group) => <LogGroupListSamples group={group} onShowLines={props.onShowLines} />}
    />
  );
}
