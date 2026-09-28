// What a number means and how it reads: every chart, table, and stat takes a
// unit by name, so a dashboard can name it in JSON.

/**
 * - `count`: things counted, `1,284` or `12.9K`.
 * - `duration`: nanoseconds, in the units of the CLI, `4.56ms` or `3m05s`.
 * - `ratio`: a share from 0 to 1, as a percentage, `2.67%`.
 * - `rate`: events per second, `0.42/s`.
 * - `bytes`: `512B`, `1.5KiB`, `3.2GiB`.
 * - `number`: any other number, to three significant digits.
 */
export type Unit = "count" | "duration" | "ratio" | "rate" | "bytes" | "number";

/** One number of a value and the unit after it, such as `4.56` and `ms`. A
 * duration of a minute and more has two parts: `3` `m`, `05` `s`. */
export interface ValuePart {
  value: string;
  unit: string;
  /** The unit sticks to the number without a gap, such as `%` or `K`. */
  tight?: boolean;
}

/** A number with three significant digits at most, without trailing zeros. */
const significant = (n: number, digits = 3) => String(Number(n.toPrecision(digits)));

const pad = (n: number) => String(n).padStart(2, "0");

/** The units under a minute, each a thousand of the one before it. */
const SMALL_UNITS = [
  { unit: "µs", size: 1e3 },
  { unit: "ms", size: 1e6 },
  { unit: "s", size: 1e9 },
];

/** A duration in nanoseconds in the units of the CLI, to three significant
 * digits: `850ns`, `12.3µs`, `4.56ms`, `1.2s`, or two parts at a minute and
 * more, `3m05s` or `2h10m`. */
export function durationParts(nanos: number): ValuePart[] {
  const size = Math.abs(nanos);
  if (size === 0) return [{ value: "0", unit: "s" }];
  if (size < 1e3) return [{ value: String(Math.round(nanos)), unit: "ns" }];
  // A value that rounds up to 1000 goes to the next unit: 999.9µs is 1ms.
  const small = SMALL_UNITS.find(
    ({ size: unit }) => Math.abs(Number((nanos / unit).toPrecision(3))) < 1000,
  );
  if (small && size < 60e9) {
    const value = Number((nanos / small.size).toPrecision(3));
    if (small.unit !== "s" || Math.abs(value) < 60)
      return [{ value: String(value), unit: small.unit }];
  }
  const seconds = Math.round(nanos / 1e9);
  if (Math.abs(seconds) < 3600) {
    const rest = seconds % 60;
    const minutes = { value: String(Math.trunc(seconds / 60)), unit: "m" };
    return rest === 0 ? [minutes] : [minutes, { value: pad(rest), unit: "s" }];
  }
  const minutes = Math.round(nanos / 60e9);
  const rest = minutes % 60;
  const hours = { value: String(Math.trunc(minutes / 60)), unit: "h" };
  return rest === 0 ? [hours] : [hours, { value: pad(rest), unit: "m" }];
}

/** `1,284`, or `12.9K`, `4.2M`, `1.1B` from ten thousand up. */
function countParts(n: number): ValuePart[] {
  const size = Math.abs(n);
  if (size < 10_000) {
    const value = Number.isInteger(n) ? n.toLocaleString("en-US") : significant(n);
    return [{ value, unit: "" }];
  }
  if (size < 999_500) return [{ value: significant(n / 1e3), unit: "K", tight: true }];
  if (size < 999_500_000) return [{ value: significant(n / 1e6), unit: "M", tight: true }];
  return [{ value: significant(n / 1e9), unit: "B", tight: true }];
}

/** A share as a percentage. A share above zero never shows as `0%`, and one
 * below 1 never as `100%`. */
function ratioParts(share: number): ValuePart[] {
  const percent = share * 100;
  let value: string;
  if (percent === 0) value = "0";
  else if (percent < 0.01) value = "<0.01";
  else if (percent < 1) value = significant(percent, 2);
  else if (percent < 99.95) value = significant(percent);
  else value = percent >= 100 ? significant(percent) : ">99.9";
  return [{ value, unit: "%", tight: true }];
}

/** A rate per second: `12.3/s`, `0.05/s`, `<0.01/s`, or `0/s`. */
function rateParts(rate: number): ValuePart[] {
  let value: string;
  if (rate === 0) value = "0";
  else if (rate < 0.01) value = "<0.01";
  else if (rate < 1) value = rate.toFixed(2).replace(/0$/, "");
  else if (rate < 10_000) value = significant(rate);
  else {
    const [compact] = countParts(rate);
    return [{ value: compact?.value ?? "", unit: `${compact?.unit ?? ""}/s`, tight: true }];
  }
  return [{ value, unit: "/s", tight: true }];
}

const BYTE_UNITS = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

function byteParts(bytes: number): ValuePart[] {
  let value = bytes;
  let i = 0;
  while (Math.abs(value) >= 1024 && i < BYTE_UNITS.length - 1) {
    value /= 1024;
    i++;
  }
  return [
    { value: i === 0 ? String(Math.round(value)) : significant(value), unit: BYTE_UNITS[i] ?? "" },
  ];
}

const PARTS: Record<Unit, (value: number) => ValuePart[]> = {
  count: countParts,
  duration: durationParts,
  ratio: ratioParts,
  rate: rateParts,
  bytes: byteParts,
  number: (value) => [{ value: significant(value), unit: "" }],
};

/** The parts of `value` in `unit`. */
export const valueParts = (value: number, unit: Unit): ValuePart[] =>
  Number.isFinite(value) ? PARTS[unit](value) : [{ value: "–", unit: "" }];

/** `value` in `unit` as text, such as `4.56ms` or `2.67%`. A missing value
 * is a dash. */
export const formatValue = (value: number | null | undefined, unit: Unit) =>
  value === null || value === undefined
    ? "–"
    : valueParts(value, unit)
        .map((part) => part.value + part.unit)
        .join("");

/** A duration in nanoseconds as text, such as `4.56ms` or `3m05s`. */
export const formatDuration = (nanos: number) => formatValue(nanos, "duration");
