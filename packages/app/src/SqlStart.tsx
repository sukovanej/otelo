import { lexSql } from "./sql";
import SqlTokens from "./sql-tokens";

const SQL_START_MAX_CHARS = 300;

interface SqlStartProps {
  readonly text: string;
}

export default function SqlStart(props: SqlStartProps) {
  return <SqlTokens tokens={lexSql(props.text.slice(0, SQL_START_MAX_CHARS))} />;
}
