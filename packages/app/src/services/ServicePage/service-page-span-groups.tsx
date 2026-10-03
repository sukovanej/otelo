import { createMemo } from "solid-js";

import type { SpanGroup, SpanGroups } from "@otelo/api";
import { type Column, Panel, Table } from "@otelo/viz";

import SpanTitle from "../../traces/SpanTitle";
import { PERCENTILES } from "../series";
import { measureSeconds, toRate, toShare } from "../stats";

interface ServicePageSpanGroupsProps {
  readonly title: string;
  readonly description: string;
  readonly nameLabel: string;
  readonly countLabel: string;
  readonly groups: SpanGroups;
  readonly loading: boolean;
  readonly groupHref: (group: SpanGroup) => string;
  readonly onOpenGroup: (group: SpanGroup) => void;
}

export default function ServicePageSpanGroups(props: ServicePageSpanGroupsProps) {
  const rangeSeconds = () => measureSeconds(props.groups.start_at, props.groups.end_at);

  const columns = createMemo<Column<SpanGroup>[]>(() => [
    {
      kind: "cell",
      id: "name",
      label: props.nameLabel,
      width: "minmax(24ch,5fr)",
      sortBy: (group) => group.name,
      cell: (group) => (
        <SpanTitle variant="group" name={group.name} attributes={group.attributes} />
      ),
    },
    {
      kind: "meter",
      id: "count",
      label: props.countLabel,
      unit: "count",
      value: (group) => group.spans.count,
    },
    {
      kind: "number",
      id: "rate",
      label: "Rate",
      unit: "rate",
      value: (group) => toRate(group.spans.count, rangeSeconds()),
    },
    {
      kind: "number",
      id: "errors",
      label: "Error rate",
      unit: "ratio",
      value: (group) => toShare(group.spans.errors, group.spans.count),
      tone: (group) => (group.spans.errors > 0 ? "error" : undefined),
    },
    ...PERCENTILES.map((percentile): Column<SpanGroup> => ({
      kind: "number",
      id: percentile,
      label: percentile.toUpperCase(),
      unit: "duration",
      value: (group) => group.spans.latency?.[percentile] ?? null,
    })),
    {
      kind: "meter",
      id: "total",
      label: "Total time",
      description: "The durations of its spans added up",
      unit: "duration",
      value: (group) => group.spans.total_ns,
    },
  ]);

  return (
    <Panel
      title={props.title}
      description={
        props.groups.truncated
          ? `${props.description}; the ones with the most time only`
          : props.description
      }
      flush
    >
      <Table
        label={props.title}
        rows={props.groups.groups}
        rowKey={(group) => JSON.stringify(group.values)}
        columns={columns()}
        sorting={{ kind: "table", initialOrder: { columnId: "total", descending: true } }}
        href={props.groupHref}
        onRowClick={props.onOpenGroup}
        loading={props.loading}
      />
    </Panel>
  );
}
