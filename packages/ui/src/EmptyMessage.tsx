import type { JSX } from "@solidjs/web";

interface EmptyMessageProps {
  readonly children: JSX.Element;
}

export default function EmptyMessage(props: EmptyMessageProps) {
  return <div class="py-8 text-center text-muted">{props.children}</div>;
}
