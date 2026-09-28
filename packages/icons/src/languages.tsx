// The icons of programming languages: simple drawings in the colors each
// language is known by, which read at 14 pixels on a light and a dark page.

import type { Component } from "solid-js";
import { Dynamic } from "solid-js/web";
import Icon, { type IconProps } from "./Icon";
import { CodeIcon } from "./ui";

const WHITE = "#ffffff";
const INK = "#1c2027";
const RUST = "#f74c00";

/** A crab, orange, with its claws up, after Ferris, the crab of Rust. */
export function RustIcon(props: IconProps) {
  // A function, since an element is one node, which a second use would move.
  const half = () => (
    <>
      {/* A claw, open, on an arm that reaches down to the shell. */}
      <path d="M1.1 5.9C.5 4.6 1 3 2.4 2.7l.1 1.5 1.1-.7c.5 1.3-.1 2.6-1.3 2.9Z" fill={RUST} />
      <path
        d="M2.5 6 3.6 8.3M4.3 12l-1.3 1.7M6 12.4l-.5 1.9"
        fill="none"
        stroke={RUST}
        stroke-width="1.1"
        stroke-linecap="round"
      />
    </>
  );
  return (
    <Icon {...props}>
      {half()}
      <g transform="matrix(-1 0 0 1 16 0)">{half()}</g>
      <path
        d="M2.6 10.3C2.6 7.2 5 5.4 8 5.4s5.4 1.8 5.4 4.9c0 1.1-.8 1.8-2 1.8H4.6c-1.2 0-2-.7-2-1.8Z"
        fill={RUST}
      />
      <circle cx="6.4" cy="8.3" r="1" fill={INK} />
      <circle cx="9.6" cy="8.3" r="1" fill={INK} />
      <circle cx="6.7" cy="8" r="0.35" fill={WHITE} />
      <circle cx="9.9" cy="8" r="0.35" fill={WHITE} />
      <path
        d="M7.1 10.2c.5.4 1.3.4 1.8 0"
        fill="none"
        stroke={INK}
        stroke-width="0.6"
        stroke-linecap="round"
      />
    </Icon>
  );
}

/** GO in cyan, after lines of speed. */
export function GoIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g fill="none" stroke="#00add8" stroke-width="1.4" stroke-linecap="round">
        <path d="M0.8 6.4h2.4M0.4 8.3h2.2M1 10.2h1.6" />
        <path d="M8.7 6.3a2.4 2.4 0 1 0 .4 2.3H7.5" />
        <circle cx="12.9" cy="8" r="2.35" />
      </g>
    </Icon>
  );
}

/** Two snakes, a blue one and a yellow one, head to tail. */
export function PythonIcon(props: IconProps) {
  const snake =
    "M8 1.3c-2.6 0-3.6.9-3.6 2.4v1.7h3.8v.7H3.3C1.9 6.1 1.2 7.2 1.2 8.9s.8 2.9 2.1 2.9h1.2V9.9c0-1.2.9-2.1 2.1-2.1h3.1c1.1 0 2-.9 2-2V3.7c0-1.5-1.2-2.4-3.7-2.4Z";
  return (
    <Icon {...props}>
      <path d={snake} fill="#3776ab" />
      <circle cx="6.1" cy="3.1" r="0.65" fill={WHITE} />
      <g transform="rotate(180 8 8)">
        <path d={snake} fill="#ffd43b" />
        <circle cx="6.1" cy="3.1" r="0.65" fill={WHITE} />
      </g>
    </Icon>
  );
}

/** A yellow square with JS. */
export function JavaScriptIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="2" fill="#f7df1e" />
      <path
        d="M7.3 7.2v4.3c0 1-.6 1.6-1.5 1.6-.7 0-1.2-.4-1.4-1M12.7 7.9c-.3-.5-.8-.8-1.5-.8-.8 0-1.3.4-1.3 1s.5.9 1.4 1.2c1 .3 1.6.7 1.6 1.6s-.7 1.6-1.8 1.6c-.9 0-1.6-.4-1.9-1.1"
        fill="none"
        stroke={INK}
        stroke-width="1.25"
        stroke-linecap="round"
      />
    </Icon>
  );
}

/** A blue square with TS. */
export function TypeScriptIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="2" fill="#3178c6" />
      <path
        d="M3.4 7.4h4M5.4 7.4v5.5M12.7 7.9c-.3-.5-.8-.8-1.5-.8-.8 0-1.3.4-1.3 1s.5.9 1.4 1.2c1 .3 1.6.7 1.6 1.6s-.7 1.6-1.8 1.6c-.9 0-1.6-.4-1.9-1.1"
        fill="none"
        stroke={WHITE}
        stroke-width="1.25"
        stroke-linecap="round"
      />
    </Icon>
  );
}

/** A cup of coffee, steaming. */
export function JavaIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M6.2 1.8c-1 1 .9 1.8 0 3.1M8.8 2.3c-1 1 .9 1.8 0 3.1"
        fill="none"
        stroke="#e76f00"
        stroke-width="1.1"
        stroke-linecap="round"
      />
      <path d="M2.8 7h8.4v2.3c0 2-1.6 3.6-3.6 3.6H6.4c-2 0-3.6-1.6-3.6-3.6Z" fill="#5382a1" />
      <path
        d="M11.2 7.9h.9a1.55 1.55 0 0 1 0 3.1h-1.4M2.3 14.4h9.8"
        fill="none"
        stroke="#5382a1"
        stroke-width="1.1"
        stroke-linecap="round"
      />
    </Icon>
  );
}

/** A hexagon of `color` with a C and what follows it: `#` or `++`. */
function cHexagon(color: string, suffix: "sharp" | "plus") {
  return (props: IconProps) => (
    <Icon {...props}>
      <path d="M8 .9l6.2 3.55v7.1L8 15.1l-6.2-3.55v-7.1Z" fill={color} />
      <g fill="none" stroke={WHITE} stroke-linecap="round">
        <path d="M8.1 6.1a2.5 2.5 0 1 0 0 3.8" stroke-width="1.35" />
        {suffix === "sharp" ? (
          <path d="M10.3 6.3l-.3 3.4M11.8 6.3l-.3 3.4M9.6 7.3h2.8M9.4 8.7h2.8" stroke-width="0.8" />
        ) : (
          <path d="M9.4 8h2M10.4 7v2M11.9 8h2M12.9 7v2" stroke-width="0.9" />
        )}
      </g>
    </Icon>
  );
}

/** A purple hexagon with C#. */
export const CSharpIcon = cHexagon("#512bd4", "sharp");

/** A blue hexagon with C++. */
export const CppIcon = cHexagon("#00599c", "plus");

/** A red gem, cut. */
export function RubyIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4.4 2.6h7.2l2.8 3.7L8 14 1.6 6.3Z" fill="#cc342d" />
      <path
        d="M1.6 6.3h12.8M4.4 2.6l1.5 3.7L8 2.6l2.1 3.7 1.5-3.7M5.9 6.3 8 14l2.1-7.7"
        fill="none"
        stroke={WHITE}
        stroke-opacity="0.45"
        stroke-width="0.7"
        stroke-linejoin="round"
      />
    </Icon>
  );
}

/** A purple oval with php. */
export function PhpIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <ellipse cx="8" cy="8" rx="7.4" ry="5" fill="#777bb4" />
      <path
        d="M2.9 10.8V6.2h1.4a1.1 1.1 0 0 1 0 2.2H2.9M6.4 5.4v4.1M6.4 7.6c.3-.5.7-.8 1.2-.8.6 0 .9.4.9 1v1.7M10.5 10.8V6.2h1.4a1.1 1.1 0 0 1 0 2.2h-1.4"
        fill="none"
        stroke={WHITE}
        stroke-width="1"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </Icon>
  );
}

/** A white bird on orange. */
export function SwiftIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="3.2" fill="#f05138" />
      <path
        d="M12.6 10.4c.5-1.8-.2-4.1-2-5.9.9 1.6 1.1 3.4.5 4.6C9.3 7.9 7 6.2 4.7 4.3c1.5 1.7 2.8 3 4 4.2C7.1 7.6 5.4 6.4 3.8 5.3c1.6 2.1 3.4 4 5.4 5.3-1.5.6-3.3.6-5.2-.2 1.8 1.5 4 2.1 5.9 1.7 1-.2 1.8-.6 2.5-.3.4.1.7.4.8.8.4-.8.2-1.7-.6-2.2Z"
        fill={WHITE}
      />
    </Icon>
  );
}

/** A red square with an e, for Erlang and the languages of its VM. */
export function ErlangIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="2" fill="#a90533" />
      <path
        d="M4.4 8.3h7.2c0-2.1-1.5-3.5-3.5-3.5S4.4 6.2 4.4 8.3s1.5 3.5 3.7 3.5c1.4 0 2.5-.5 3.2-1.4"
        fill="none"
        stroke={WHITE}
        stroke-width="1.4"
        stroke-linecap="round"
      />
    </Icon>
  );
}

/** Each language with an icon, by a plain id, with the name to show. */
export const LANGUAGES: Record<string, { name: string; icon: Component<IconProps> }> = {
  rust: { name: "Rust", icon: RustIcon },
  go: { name: "Go", icon: GoIcon },
  python: { name: "Python", icon: PythonIcon },
  javascript: { name: "JavaScript", icon: JavaScriptIcon },
  typescript: { name: "TypeScript", icon: TypeScriptIcon },
  java: { name: "Java", icon: JavaIcon },
  csharp: { name: "C#", icon: CSharpIcon },
  cpp: { name: "C++", icon: CppIcon },
  ruby: { name: "Ruby", icon: RubyIcon },
  php: { name: "PHP", icon: PhpIcon },
  swift: { name: "Swift", icon: SwiftIcon },
  erlang: { name: "Erlang", icon: ErlangIcon },
};

/** The name of a language by its id in `LANGUAGES`, such as `JavaScript`
 * for `javascript`, or the id for a language it does not know. */
export const languageName = (id: string | undefined) =>
  id === undefined ? "Unknown language" : (LANGUAGES[id]?.name ?? id);

/** The icon of a language by its id in `LANGUAGES`, titled with its name,
 * or the code icon for a language it does not know, titled with the id. */
export function LanguageIcon(props: IconProps & { language: string | undefined }) {
  const known = () => (props.language === undefined ? undefined : LANGUAGES[props.language]);
  return (
    <Dynamic
      component={known()?.icon ?? CodeIcon}
      size={props.size}
      class={known() ? props.class : `text-muted ${props.class ?? ""}`}
      title={props.title ?? languageName(props.language)}
    />
  );
}
