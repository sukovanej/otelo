import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

type CSuffix = "sharp" | "plus";

interface CHexagonIconProps extends IconProps {
  readonly color: string;
  readonly suffix: CSuffix;
}

export default function CHexagonIcon(props: CHexagonIconProps) {
  return (
    <Icon {...props}>
      <path d="M8 .9l6.2 3.55v7.1L8 15.1l-6.2-3.55v-7.1Z" fill={props.color} />
      <g fill="none" stroke={WHITE} stroke-linecap="round">
        <path d="M8.1 6.1a2.5 2.5 0 1 0 0 3.8" stroke-width="1.35" />
        {props.suffix === "sharp" ? (
          <path d="M10.3 6.3l-.3 3.4M11.8 6.3l-.3 3.4M9.6 7.3h2.8M9.4 8.7h2.8" stroke-width="0.8" />
        ) : (
          <path d="M9.4 8h2M10.4 7v2M11.9 8h2M12.9 7v2" stroke-width="0.9" />
        )}
      </g>
    </Icon>
  );
}
