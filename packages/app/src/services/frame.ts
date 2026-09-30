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
    bucketStartsMs: answer.buckets.map((bucket) => parseTime(bucket.start_at).getTime()),
    stepMs: answer.step_ns / 1e6,
    startMs: parseTime(answer.start_at).getTime(),
    endMs: parseTime(answer.end_at).getTime(),
  };
}
