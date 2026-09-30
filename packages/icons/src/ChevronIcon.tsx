import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

const TURN_DEGREES: Record<ChevronDirection, number> = { down: 0, right: -90, up: 180, left: 90 };

type ChevronDirection = "down" | "right" | "up" | "left";

interface ChevronIconProps extends IconProps {
  readonly direction?: ChevronDirection;
}

export default function ChevronIcon(props: ChevronIconProps) {
  return (
    <Icon {...props}>
      <path
        d="M4.5 6.5 8 10l3.5-3.5"
        transform={`rotate(${TURN_DEGREES[props.direction ?? "down"]} 8 8)`}
        {...LINE_STROKE}
        stroke-width="1.6"
      />
    </Icon>
  );
}
