import { Badge, type BadgeTone } from "@otelo/ui";

interface HttpStatusBadgeProps {
  readonly status: number;
}

export default function HttpStatusBadge(props: HttpStatusBadgeProps) {
  return <Badge tone={toStatusTone(props.status)}>{props.status}</Badge>;
}

function toStatusTone(status: number): BadgeTone {
  if (status >= 500) return "error";
  if (status >= 400) return "warn";
  if (status >= 300) return "info";
  if (status >= 200) return "success";
  return "muted";
}
