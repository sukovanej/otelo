import type { JSX } from "solid-js";

import { RangePicker } from "@siner/ui";

import type { Fetched } from "../fetch";
import PageBar, { LiveToggle } from "../PageBar";
import type { Range } from "./range";

/**
 * The bar at the top of a services page, in the frame of every page but in
 * one row: the name of what the page shows (`title`) and what the answer
 * holds (`children`), then the range and live mode.
 */
export default function Toolbar(props: {
  range: Range;
  fetched: Fetched<unknown>;
  title: JSX.Element;
  children?: JSX.Element;
}) {
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
