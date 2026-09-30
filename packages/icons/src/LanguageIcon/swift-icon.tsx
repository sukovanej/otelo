import { WHITE } from "../colors";
import Icon, { type IconProps } from "../icon";

export default function SwiftIcon(props: IconProps) {
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
