import Icon, { type IconProps } from "../icon";
import { LINE_STROKE } from "../line-stroke";

export default function DatabaseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <g {...LINE_STROKE} stroke-width="1.2">
        <ellipse cx="8" cy="3.8" rx="5.3" ry="2.1" />
        <path d="M2.7 3.8v8.4c0 1.16 2.37 2.1 5.3 2.1s5.3-.94 5.3-2.1V3.8" />
        <path d="M2.7 8c0 1.16 2.37 2.1 5.3 2.1s5.3-.94 5.3-2.1" />
      </g>
    </Icon>
  );
}
