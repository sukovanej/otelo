import { SqlCode, SqlStart, Tooltip } from "@otelo/ui";

const TOOLTIP_MAX_LINES = 24;

interface SqlQueryStartProps {
  readonly query: string;
}

export default function SqlQueryStart(props: SqlQueryStartProps) {
  return (
    <Tooltip
      rich
      content={<SqlCode text={props.query} maxLines={TOOLTIP_MAX_LINES} />}
      class="min-w-0"
    >
      <span class="truncate">
        <SqlStart text={props.query} />
      </span>
    </Tooltip>
  );
}
