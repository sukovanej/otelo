import Icon, { type IconProps } from "./icon";
import { LINE_STROKE } from "./line-stroke";

export default function CloseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 4l8 8M12 4l-8 8" {...LINE_STROKE} />
    </Icon>
  );
}
