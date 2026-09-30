import { createSignal } from "solid-js";

import type { LogGroup } from "@otelo/api";
import { Level } from "@otelo/ui";
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
      id: "count",
      label: "Lines",
      unit: "count",
      meter: true,
      width: "16ch",
      value: (group) => group.count,
    },
    {
      id: "level",
      label: "Level",
      width: "max-content",
      value: (group) => toLevelName(group.severity),
      cell: (group) => <Level level={toLevelName(group.severity)} />,
    },
    {
      id: "template",
      label: "Template",
      width: "minmax(0,1fr)",
      value: (group) => group.template,
      cell: (group) => (
        <span class="wrap-anywhere">
          <LogGroupListTemplate text={group.template} />
        </span>
      ),
    },
    {
      id: "services",
      label: "Services",
      width: "minmax(8ch,20ch)",
      value: (group) => group.services.join(", "),
      tone: () => "muted",
    },
    {
      id: "last",
      label: "Last",
      align: "end",
      width: "max-content",
      value: (group) => group.last_at,
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
      columns={columns}
      selected={(group) => openTemplate() === group.template}
      onRowClick={(group) =>
        setOpenTemplate(openTemplate() === group.template ? undefined : group.template)
      }
      expanded={(group) => openTemplate() === group.template}
      detail={(group) => <LogGroupListSamples group={group} onShowLines={props.onShowLines} />}
    />
  );
}
