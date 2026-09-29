import type { TimeFrame } from "@siner/viz";

import { parseTime } from "../time";

/** The frame of the charts of an answer of the services API. */
export function timeFrame(answer: {
  start_at: string;
  end_at: string;
  step_ns: number;
  buckets: { start_at: string }[];
}): TimeFrame {
  return {
    times: answer.buckets.map((b) => parseTime(b.start_at).getTime()),
    step: answer.step_ns / 1e6,
    start: parseTime(answer.start_at).getTime(),
    end: parseTime(answer.end_at).getTime(),
  };
}
