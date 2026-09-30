import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function RedisIcon(props: IconProps) {
  return (
    <Icon {...props}>
      {drawSlab(7.6, "#a41e11")}
      {drawSlab(4.8, "#d82c20")}
      {drawSlab(2, "#ff4438")}
      <path d="M8 3.6l1.5 1.1L8 5.8 6.5 4.7Z" fill={WHITE} />
    </Icon>
  );
}

function drawSlab(topY: number, color: string) {
  return <path d={`M8 ${topY}l6.2 2.7L8 ${topY + 5.4}l-6.2-2.7Z`} fill={color} />;
}
