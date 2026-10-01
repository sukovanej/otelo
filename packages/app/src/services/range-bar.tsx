import type { JSX } from "@solidjs/web";

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
            since={props.range.since()}
            until={props.range.until()}
            onChange={(since, until) => props.range.setRange(since, until)}
          />
          <LiveToggle
            live={props.range.live()}
            until={props.range.until()}
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
