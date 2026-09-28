// The numbers of the services pages that the API leaves to the UI.

import { parseTime } from "../time";

/** The seconds from `since` to `until`, both RFC 3339. */
export const seconds = (since: string, until: string) =>
  Math.max(0, (parseTime(until).getTime() - parseTime(since).getTime()) / 1000);

/** The share of `part` in `whole`, from 0 to 1, or `null` for an empty
 * whole. */
export const share = (part: number, whole: number) => (whole > 0 ? part / whole : null);

/** Events per second over `overSeconds`, or `null` for an empty range. */
export const rate = (count: number, overSeconds: number) =>
  overSeconds > 0 ? count / overSeconds : null;
