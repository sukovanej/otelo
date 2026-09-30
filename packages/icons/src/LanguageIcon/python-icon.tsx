import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

const SNAKE_PATH =
  "M8 1.3c-2.6 0-3.6.9-3.6 2.4v1.7h3.8v.7H3.3C1.9 6.1 1.2 7.2 1.2 8.9s.8 2.9 2.1 2.9h1.2V9.9c0-1.2.9-2.1 2.1-2.1h3.1c1.1 0 2-.9 2-2V3.7c0-1.5-1.2-2.4-3.7-2.4Z";

export default function PythonIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d={SNAKE_PATH} fill="#3776ab" />
      <circle cx="6.1" cy="3.1" r="0.65" fill={WHITE} />
      <g transform="rotate(180 8 8)">
        <path d={SNAKE_PATH} fill="#ffd43b" />
        <circle cx="6.1" cy="3.1" r="0.65" fill={WHITE} />
      </g>
    </Icon>
  );
}
