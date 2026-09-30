import type { TimeFrame } from "@otelo/viz";

import { parseTime } from "../time";

interface BucketStart {
  readonly start_at: string;
}

interface BucketedAnswer {
  readonly start_at: string;
  readonly end_at: string;
  readonly step_ns: number;
  readonly buckets: ReadonlyArray<BucketStart>;
}

export function toTimeFrame(answer: BucketedAnswer): TimeFrame {
  return {
    times: answer.buckets.map((bucket) => parseTime(bucket.start_at).getTime()),
    step: answer.step_ns / 1e6,
    start: parseTime(answer.start_at).getTime(),
    end: parseTime(answer.end_at).getTime(),
  };
}
