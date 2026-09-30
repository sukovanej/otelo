import type { Component } from "solid-js";
import { Dynamic } from "solid-js/web";

import type { IconProps } from "../icon";
import CassandraIcon from "./cassandra-icon";
import ClickHouseIcon from "./click-house-icon";
import DatabaseIcon from "./database-icon";
import ElasticsearchIcon from "./elasticsearch-icon";
import MariaDbIcon from "./maria-db-icon";
import MongoDbIcon from "./mongo-db-icon";
import MySqlIcon from "./my-sql-icon";
import OracleIcon from "./oracle-icon";
import PostgresIcon from "./postgres-icon";
import RedisIcon from "./redis-icon";
import SqlServerIcon from "./sql-server-icon";
import SqliteIcon from "./sqlite-icon";

const DATABASE_SYSTEMS: Record<string, DatabaseSystem> = {
  postgresql: { name: "PostgreSQL", icon: PostgresIcon },
  mysql: { name: "MySQL", icon: MySqlIcon },
  mariadb: { name: "MariaDB", icon: MariaDbIcon },
  sqlite: { name: "SQLite", icon: SqliteIcon },
  redis: { name: "Redis", icon: RedisIcon },
  mongodb: { name: "MongoDB", icon: MongoDbIcon },
  mssql: { name: "SQL Server", icon: SqlServerIcon },
  oracle: { name: "Oracle", icon: OracleIcon },
  elasticsearch: { name: "Elasticsearch", icon: ElasticsearchIcon },
  clickhouse: { name: "ClickHouse", icon: ClickHouseIcon },
  cassandra: { name: "Cassandra", icon: CassandraIcon },
};

interface DatabaseSystem {
  readonly name: string;
  readonly icon: Component<IconProps>;
}

interface DatabaseSystemIconProps extends IconProps {
  readonly system: string;
}

export default function DatabaseSystemIcon(props: DatabaseSystemIconProps) {
  const knownSystem = () => DATABASE_SYSTEMS[props.system];
  return (
    <Dynamic
      component={knownSystem()?.icon ?? DatabaseIcon}
      size={props.size}
      class={knownSystem() ? props.class : `text-database ${props.class ?? ""}`}
      title={props.title ?? databaseName(props.system)}
    />
  );
}

export function databaseName(systemId: string): string {
  return DATABASE_SYSTEMS[systemId]?.name ?? systemId;
}
