import { createMemo } from "solid-js";

import type { SpanGroup, SpanGroups } from "@otelo/api";
import { ChartPanel, type TimeSeries } from "@otelo/viz";

import { toTimeFrame } from "../frame";
import { toCountSeries } from "../series";
import ServicePageSpanGroups from "./service-page-span-groups";

interface ServicePageQueriesProps {
  readonly queries: SpanGroups;
  readonly loading: boolean;
  readonly onZoom: (start: number, end: number) => void;
  readonly queryHref: (group: SpanGroup) => string;
  readonly onOpenQuery: (group: SpanGroup) => void;
}

export default function ServicePageQueries(props: ServicePageQueriesProps) {
  const frame = createMemo(() => toTimeFrame(props.queries));
  const countSeries = createMemo(() =>
    toCountSeries(props.queries.buckets.map((bucket) => bucket.spans)),
  );
  const timeSeries = createMemo<TimeSeries[]>(() => [
    {
      label: "Time",
      color: "series-1",
      values: props.queries.buckets.map((bucket) => bucket.spans.total_ns),
    },
  ]);

  return (
    <>
      <div class="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <ChartPanel
          title="Time in queries"
          description="The durations of the database queries added up, by step"
          kind="bar"
          unit="duration"
          frame={frame()}
          series={timeSeries()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
        <ChartPanel
          title="Query count"
          description="Database queries, by step"
          kind="bar"
          unit="count"
          frame={frame()}
          series={countSeries()}
          loading={props.loading}
          onZoom={props.onZoom}
        />
      </div>

      <ServicePageSpanGroups
        title="Queries"
        description="The database queries by database and query text"
        nameLabel="Query"
        countLabel="Queries"
        groups={props.queries}
        loading={props.loading}
        groupHref={props.queryHref}
        onOpenGroup={props.onOpenQuery}
      />
    </>
  );
}
