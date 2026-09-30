const SUBMINUTE_UNITS: ReadonlyArray<DurationUnit> = [
  { unit: "µs", nanos: 1e3 },
  { unit: "ms", nanos: 1e6 },
  { unit: "s", nanos: 1e9 },
];

const BYTE_UNITS = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

const VALUE_SPLITTERS: Record<Unit, (value: number) => ValuePart[]> = {
  count: splitCount,
  duration: splitDuration,
  ratio: splitRatio,
  rate: splitRate,
  bytes: splitBytes,
  "bytes-per-second": splitBytesPerSecond,
  number: (value) => [{ value: formatSignificant(value), unit: "" }],
};

export type Unit =
  | "count"
  | "duration"
  | "ratio"
  | "rate"
  | "bytes"
  | "bytes-per-second"
  | "number";

interface ValuePart {
  readonly value: string;
  readonly unit: string;
  readonly unitAttached?: boolean;
}

interface DurationUnit {
  readonly unit: string;
  readonly nanos: number;
}

export function splitValue(value: number, unit: Unit): ValuePart[] {
  return Number.isFinite(value) ? VALUE_SPLITTERS[unit](value) : [{ value: "–", unit: "" }];
}

export function formatValue(value: number | null | undefined, unit: Unit): string {
  return value === null || value === undefined
    ? "–"
    : splitValue(value, unit)
        .map((part) => part.value + part.unit)
        .join("");
}

function splitDuration(nanos: number): ValuePart[] {
  const absoluteNanos = Math.abs(nanos);
  if (absoluteNanos === 0) return [{ value: "0", unit: "s" }];
  if (absoluteNanos < 1e3) return [{ value: String(Math.round(nanos)), unit: "ns" }];
  // A value that rounds up to 1000 goes to the next unit: 999.9µs is 1ms.
  const subminuteUnit = SUBMINUTE_UNITS.find(
    (durationUnit) => Math.abs(Number((nanos / durationUnit.nanos).toPrecision(3))) < 1000,
  );
  if (subminuteUnit && absoluteNanos < 60e9) {
    const value = Number((nanos / subminuteUnit.nanos).toPrecision(3));
    if (subminuteUnit.unit !== "s" || Math.abs(value) < 60) {
      return [{ value: String(value), unit: subminuteUnit.unit }];
    }
  }
  const totalSeconds = Math.round(nanos / 1e9);
  if (Math.abs(totalSeconds) < 3600) {
    const restSeconds = totalSeconds % 60;
    const minutesPart = { value: String(Math.trunc(totalSeconds / 60)), unit: "m" };
    return restSeconds === 0
      ? [minutesPart]
      : [minutesPart, { value: padToTwoDigits(restSeconds), unit: "s" }];
  }
  const totalMinutes = Math.round(nanos / 60e9);
  const restMinutes = totalMinutes % 60;
  const hoursPart = { value: String(Math.trunc(totalMinutes / 60)), unit: "h" };
  return restMinutes === 0
    ? [hoursPart]
    : [hoursPart, { value: padToTwoDigits(restMinutes), unit: "m" }];
}

function splitCount(count: number): ValuePart[] {
  const absoluteCount = Math.abs(count);
  if (absoluteCount < 10_000) {
    const value = Number.isInteger(count)
      ? count.toLocaleString("en-US")
      : formatSignificant(count);
    return [{ value, unit: "" }];
  }
  if (absoluteCount < 999_500) {
    return [{ value: formatSignificant(count / 1e3), unit: "K", unitAttached: true }];
  }
  if (absoluteCount < 999_500_000) {
    return [{ value: formatSignificant(count / 1e6), unit: "M", unitAttached: true }];
  }
  return [{ value: formatSignificant(count / 1e9), unit: "B", unitAttached: true }];
}

function splitRatio(share: number): ValuePart[] {
  const percent = share * 100;
  let value: string;
  if (percent === 0) value = "0";
  else if (percent < 0.01) value = "<0.01";
  else if (percent < 1) value = formatSignificant(percent, 2);
  else if (percent < 99.95) value = formatSignificant(percent);
  else value = percent >= 100 ? formatSignificant(percent) : ">99.9";
  return [{ value, unit: "%", unitAttached: true }];
}

function splitRate(rate: number): ValuePart[] {
  let value: string;
  if (rate === 0) value = "0";
  else if (rate < 0.01) value = "<0.01";
  else if (rate < 1) value = rate.toFixed(2).replace(/0$/, "");
  else if (rate < 10_000) value = formatSignificant(rate);
  else {
    const [compact] = splitCount(rate);
    return [{ value: compact?.value ?? "", unit: `${compact?.unit ?? ""}/s`, unitAttached: true }];
  }
  return [{ value, unit: "/s", unitAttached: true }];
}

function splitBytes(bytes: number): ValuePart[] {
  let value = bytes;
  let unitIndex = 0;
  while (Math.abs(value) >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  return [
    {
      value: unitIndex === 0 ? String(Math.round(value)) : formatSignificant(value),
      unit: BYTE_UNITS[unitIndex] ?? "",
    },
  ];
}

function splitBytesPerSecond(bytesPerSecond: number): ValuePart[] {
  const [bytes] = splitBytes(bytesPerSecond);
  return [{ value: bytes?.value ?? "", unit: `${bytes?.unit ?? ""}/s` }];
}

function formatSignificant(number: number, digits = 3): string {
  return String(Number(number.toPrecision(digits)));
}

function padToTwoDigits(number: number): string {
  return String(number).padStart(2, "0");
}
