import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function PlusIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3.5 8h9M8 3.5v9" {...LINE_STROKE} />
    </Icon>
  );
}
