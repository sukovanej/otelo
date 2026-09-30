import { INK, WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

const RUST_ORANGE = "#f74c00";

export default function RustIcon(props: IconProps) {
  // A function, since an element is one node, which a second use would move.
  const drawLimbs = () => (
    <>
      <path
        d="M1.1 5.9C.5 4.6 1 3 2.4 2.7l.1 1.5 1.1-.7c.5 1.3-.1 2.6-1.3 2.9Z"
        fill={RUST_ORANGE}
      />
      <path
        d="M2.5 6 3.6 8.3M4.3 12l-1.3 1.7M6 12.4l-.5 1.9"
        fill="none"
        stroke={RUST_ORANGE}
        stroke-width="1.1"
        stroke-linecap="round"
      />
    </>
  );
  return (
    <Icon {...props}>
      {drawLimbs()}
      <g transform="matrix(-1 0 0 1 16 0)">{drawLimbs()}</g>
      <path
        d="M2.6 10.3C2.6 7.2 5 5.4 8 5.4s5.4 1.8 5.4 4.9c0 1.1-.8 1.8-2 1.8H4.6c-1.2 0-2-.7-2-1.8Z"
        fill={RUST_ORANGE}
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
