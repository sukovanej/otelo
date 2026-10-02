import type { JSX } from "@solidjs/web";
import { latest } from "solid-js";

import { RangePicker } from "@otelo/ui";

import type { FetchState } from "../fetch";
import LiveToggle from "../LiveToggle";
import PageBar from "../PageBar";
import type { RangeState } from "./range";

interface RangeBarProps {
  readonly range: RangeState;
  readonly fetched: FetchState<unknown>;
  readonly title: JSX.Element;
  readonly children: JSX.Element;
}

export default function RangeBar(props: RangeBarProps) {
  return (
    <PageBar
      fetched={props.fetched}
      end={
        <>
          <RangePicker
            since={latest(props.range.since)}
            until={latest(props.range.until)}
            onChange={(since, until) => props.range.setRange(since, until)}
          />
          <LiveToggle
            live={latest(props.range.live)}
            until={latest(props.range.until)}
            onChange={(live) => props.range.setLive(live)}
          />
        </>
      }
    >
      <span class="text-ink">{props.title}</span>
      {props.children}
    </PageBar>
  );
}
