// The icons of the interface, drawn in `currentColor` with strokes, so they
// take the color of the text around them.

import Icon, { type IconProps } from "./Icon";

/** The strokes of a line icon. */
const line = {
  fill: "none",
  stroke: "currentColor",
  "stroke-width": 1.4,
  "stroke-linecap": "round",
  "stroke-linejoin": "round",
} as const;

/** The mark of otelo: three bars of log lines. It is filled, not stroked. */
export function LogoIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1.5" y="2.5" width="13" height="2.4" rx="1.2" fill="currentColor" />
      <rect x="1.5" y="6.8" width="8.5" height="2.4" rx="1.2" fill="currentColor" opacity="0.7" />
      <rect x="1.5" y="11.1" width="11" height="2.4" rx="1.2" fill="currentColor" opacity="0.45" />
    </Icon>
  );
}

/** A cross, for Close. */
export function CloseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 4l8 8M12 4l-8 8" {...line} />
    </Icon>
  );
}

/** A tick, for the chosen option. */
export function CheckIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.5 8.5 6.5 11.5 12.5 4.5" {...line} stroke-width="1.6" />
    </Icon>
  );
}

const turns = { down: 0, right: -90, up: 180, left: 90 } as const;

/** A chevron that points `down` unless told otherwise, for a menu or a fold. */
export function ChevronIcon(props: IconProps & { direction?: keyof typeof turns }) {
  return (
    <Icon {...props}>
      <path
        d="M4.5 6.5 8 10l3.5-3.5"
        transform={`rotate(${turns[props.direction ?? "down"]} 8 8)`}
        {...line}
        stroke-width="1.6"
      />
    </Icon>
  );
}

/** A calendar, for a date. */
export function CalendarIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...line} stroke-width="1.2">
        <rect x="2.2" y="3.2" width="11.6" height="10.6" rx="1.8" />
        <path d="M2.2 6.6h11.6M5.4 1.8v2.6M10.6 1.8v2.6" />
      </g>
    </Icon>
  );
}

/** A clock, for a time of day. */
export function ClockIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...line} stroke-width="1.2">
        <circle cx="8" cy="8" r="6.2" />
        <path d="M8 4.6V8l2.3 1.5" />
      </g>
    </Icon>
  );
}

/** A globe, for a request over HTTP. */
export function GlobeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...line} stroke-width="1.2">
        <circle cx="8" cy="8" r="6.2" />
        <ellipse cx="8" cy="8" rx="2.6" ry="6.2" />
        <path d="M1.8 8h12.4M2.9 4.9h10.2M2.9 11.1h10.2" />
      </g>
    </Icon>
  );
}

/** A cylinder, for a database. */
export function DatabaseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...line} stroke-width="1.2">
        <ellipse cx="8" cy="3.8" rx="5.3" ry="2.1" />
        <path d="M2.7 3.8v8.4c0 1.16 2.37 2.1 5.3 2.1s5.3-.94 5.3-2.1V3.8" />
        <path d="M2.7 8c0 1.16 2.37 2.1 5.3 2.1s5.3-.94 5.3-2.1" />
      </g>
    </Icon>
  );
}

/** A bar between two ends, for a span of time that is nothing more
 * particular: a unit of work. */
export function SpanIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.5 4.5v7M13.5 4.5v7" {...line} stroke-width="1.2" />
      <rect x="4.6" y="6.4" width="6.8" height="3.2" rx="1.1" fill="currentColor" />
    </Icon>
  );
}

/** Angle brackets around a slash, for code in a language this does not
 * know. */
export function CodeIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5.2 4.6 1.8 8l3.4 3.4M10.8 4.6 14.2 8l-3.4 3.4M9.1 3.2 6.9 12.8" {...line} />
    </Icon>
  );
}
