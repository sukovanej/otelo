import { Badge, type BadgeTone } from "@otelo/ui";

const METHOD_TONES: Record<string, BadgeTone> = {
  GET: "info",
  POST: "success",
  PUT: "warn",
  PATCH: "warn",
  DELETE: "error",
};

interface HttpMethodBadgeProps {
  readonly method: string;
}

export default function HttpMethodBadge(props: HttpMethodBadgeProps) {
  return <Badge tone={METHOD_TONES[props.method] ?? "muted"}>{props.method}</Badge>;
}
