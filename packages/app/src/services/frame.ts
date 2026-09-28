import type { TimeFrame } from "@siner/viz";

import { parseTime } from "../time";

/** The frame of the charts of an answer of the services API. */
export function timeFrame(answer: {
  since: string;
  until: string;
  step_ns: number;
  buckets: { time: string }[];
}): TimeFrame {
  return {
    times: answer.buckets.map((b) => parseTime(b.time).getTime()),
    step: answer.step_ns / 1e6,
    start: parseTime(answer.since).getTime(),
    end: parseTime(answer.until).getTime(),
  };
}
