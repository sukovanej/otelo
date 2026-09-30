import { parseTime } from "../time";

export const measureSeconds = (since: string, until: string) =>
  Math.max(0, (parseTime(until).getTime() - parseTime(since).getTime()) / 1000);

export const toShare = (part: number, whole: number) => (whole > 0 ? part / whole : null);

export const toRate = (count: number, overSeconds: number) =>
  overSeconds > 0 ? count / overSeconds : null;
