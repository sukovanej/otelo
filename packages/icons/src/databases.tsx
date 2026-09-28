// The icons of database systems: simple drawings in the colors each system
// is known by, which read at 16 pixels on a light and a dark page.

import { type Component, createUniqueId } from "solid-js";
import { Dynamic } from "solid-js/web";

import Icon, { type IconProps } from "./Icon";
import { DatabaseIcon } from "./ui";

const WHITE = "#ffffff";

/** The head of a blue elephant from the front in a dark outline, white
 * lines setting its ears apart from its face, and its trunk down the middle
 * curled to the right, after Slonik of PostgreSQL. */
export function PostgresIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M8 2.6C6.6 1.3 3.9 1.1 2.4 2.4.9 3.8 1.1 6.6 2 8.9c.7 1.7 1.9 2.4 3.2 2l1.1-.5c.1 1.8.3 3.2 1.3 4 .8.6 2 .4 2.6-.3-.8-.1-1.4-.7-1.6-1.7l-.1-1.8c1.5.5 3.9.7 5.1-1 1.2-1.8 1.5-5 .1-6.8C12.4 1.2 9.6 1.3 8 2.6Z"
        fill="#336791"
        // The outline stands apart from the page: dark on a light one, light
        // on a dark one, as the page's `color-scheme` says.
        style={{ stroke: "light-dark(#0d1b2a, #9fb3c8)" }}
        stroke-width="0.8"
        stroke-linejoin="round"
      />
      <path
        d="M7.4 3.2C5.7 4.6 5.3 7.2 6.1 10.3M8.8 3.2c1.9 1.2 2.6 3.9 1.8 7.2"
        fill="none"
        stroke={WHITE}
        stroke-width="0.75"
        stroke-linecap="round"
      />
      <circle cx="7.3" cy="6.5" r="0.55" fill={WHITE} />
      <circle cx="9.6" cy="6.5" r="0.55" fill={WHITE} />
    </Icon>
  );
}

/** A teal dolphin drawn in outline, nearly upright: its beak to the left
 * at the top, its fin up to the right, and its body down to the flukes,
 * after Sakila of MySQL. */
export function MySqlIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M1.8 4.6C2.8 3 4.8 1.9 7 1.9c1.4 0 2.5.7 3.2 1.9l3.5-1.3-2.6 2.9c.5 2 .5 4.3-.1 6.6l3.2 1.9-3.3-.5-1.5 1.7.5-2.7c.2-2.2-.4-4.4-2-6C6.8 5.3 4.4 4.9 1.8 4.6Z"
        fill="#00758f"
        fill-opacity="0.18"
        stroke="#00758f"
        stroke-width="1.2"
        stroke-linejoin="round"
      />
      <path
        d="M8.5 7.4 6.9 9.2"
        fill="none"
        stroke="#00758f"
        stroke-width="1.2"
        stroke-linecap="round"
      />
      <circle cx="5.3" cy="3.4" r="0.6" fill="#00758f" />
    </Icon>
  );
}

/** A seal lying down with its head up, a flipper out, and whiskers, after
 * the one of MariaDB. */
export function MariaDbIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M1.5 13.8c0-2.8 2.5-4.4 5.5-4.8 1-3 2.5-5 4.6-5 1.8 0 3 1 3 2.2 0 1-.8 1.6-1.8 1.6-.8 0-1.2.6-1.4 1.6-.4 2.2-1.4 4-3 4.4Z"
        fill="#c0765a"
      />
      <path d="M8.6 12.2l2.8 2.2-3.6-.2Z" fill="#c0765a" />
      <circle cx="12" cy="5.5" r="0.55" fill="#003545" />
      <circle cx="14.2" cy="6.1" r="0.45" fill="#003545" />
      <path
        d="M13.5 7l1.8.5M13.3 7.4l1.5 1"
        fill="none"
        stroke="#003545"
        stroke-width="0.4"
        stroke-linecap="round"
      />
    </Icon>
  );
}

/** A dark quill rising out of a blue square and past its corner, after
 * the feather of SQLite. */
export function SqliteIcon(props: IconProps) {
  // Every icon on the page needs its own gradient id.
  const gradient = createUniqueId();
  return (
    <Icon {...props}>
      <defs>
        <linearGradient id={gradient} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stop-color="#97d9f6" />
          <stop offset="1" stop-color="#0f80cc" />
        </linearGradient>
      </defs>
      <rect x="1" y="3.6" width="9.6" height="10.6" rx="1.8" fill={`url(#${gradient})`} />
      {/* A dark page takes the quill in a muted outline, where navy alone
          would vanish past the square. */}
      <path
        d="M14.8 1C11.5 2 8.6 5.3 7.6 9.6c-.4 1.5-.5 3-.2 4.3.6-1.5 1.4-2.9 2.6-4.1C12.7 7.1 14.5 4.3 14.8 1Z"
        fill="#003b57"
        stroke-width="0.6"
        stroke-linejoin="round"
        style={{ stroke: "light-dark(transparent, #9fb3c8)" }}
      />
      <path
        d="M7.5 13.3 7 15.3"
        stroke-width="0.9"
        stroke-linecap="round"
        style={{ stroke: "light-dark(#003b57, #9fb3c8)" }}
      />
    </Icon>
  );
}

/** One slab of the Redis icon, with its top corner at `y`. */
const slab = (y: number, color: string) => (
  <path d={`M8 ${y}l6.2 2.7L8 ${y + 5.4}l-6.2-2.7Z`} fill={color} />
);

/** Three red slabs, stacked, after the cubes of Redis. */
export function RedisIcon(props: IconProps) {
  return (
    <Icon {...props}>
      {slab(7.6, "#a41e11")}
      {slab(4.8, "#d82c20")}
      {slab(2, "#ff4438")}
      <path d="M8 3.6l1.5 1.1L8 5.8 6.5 4.7Z" fill={WHITE} />
    </Icon>
  );
}

/** A green leaf, after the one of MongoDB. */
export function MongoDbIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M8 .9c3.6 3.5 3.9 8.6.6 12.2L8.3 15h-.6l-.3-1.9C4.1 9.5 4.4 4.4 8 .9Z"
        fill="#47a248"
      />
      <path
        d="M8 2.6v12"
        fill="none"
        stroke={WHITE}
        stroke-opacity="0.55"
        stroke-width="0.6"
        stroke-linecap="round"
      />
    </Icon>
  );
}

/** A red tower of three servers, for Microsoft SQL Server. */
export function SqlServerIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2.5" y="1.3" width="11" height="13.4" rx="2" fill="#cc2927" />
      <path
        d="M2.5 5.8h11M2.5 10.2h11"
        fill="none"
        stroke={WHITE}
        stroke-opacity="0.7"
        stroke-width="0.8"
      />
      <g fill={WHITE}>
        <circle cx="11" cy="3.6" r="0.7" />
        <circle cx="11" cy="8" r="0.7" />
        <circle cx="11" cy="12.4" r="0.7" />
      </g>
    </Icon>
  );
}

/** A blue eye with lashes, after the one of Apache Cassandra. */
export function CassandraIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path
        d="M4 4.6 3 3.2M8 3.6V1.9M12 4.6l1-1.4"
        fill="none"
        stroke="#1287b1"
        stroke-width="1.1"
        stroke-linecap="round"
      />
      <path d="M1.2 9c2.6-4.4 11-4.4 13.6 0-2.6 4.4-11 4.4-13.6 0Z" fill="#1287b1" />
      <circle cx="8" cy="9" r="2.7" fill={WHITE} />
      <circle cx="8" cy="9" r="1.5" fill="#1287b1" />
      <circle cx="8.6" cy="8.4" r="0.45" fill={WHITE} />
    </Icon>
  );
}

/** A red ring, rounded at its ends, after the one of Oracle. */
export function OracleIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect
        x="1.8"
        y="4.3"
        width="12.4"
        height="7.4"
        rx="3.7"
        fill="none"
        stroke="#c74634"
        stroke-width="2.2"
      />
    </Icon>
  );
}

/** Three bands in a circle, yellow, gray, and teal, after the mark of
 * Elasticsearch. */
export function ElasticsearchIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.4 5.9a6.2 6.2 0 0 1 11.2 0Z" fill="#fec514" />
      <path d="M1.9 7h12.2a6.2 6.2 0 0 1 0 2H1.9a6.2 6.2 0 0 1 0-2Z" fill="#5c6670" />
      <path d="M2.4 10.1h11.2a6.2 6.2 0 0 1-11.2 0Z" fill="#00bfb3" />
    </Icon>
  );
}

/** Yellow bars on black, the last one short, after the mark of ClickHouse. */
export function ClickHouseIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="1" y="1" width="14" height="14" rx="2.5" fill="#1c1c1c" />
      <g fill="#faff69">
        <rect x="3" y="3.5" width="1.6" height="9" rx="0.3" />
        <rect x="5.5" y="3.5" width="1.6" height="9" rx="0.3" />
        <rect x="8" y="3.5" width="1.6" height="9" rx="0.3" />
        <rect x="10.5" y="3.5" width="1.6" height="9" rx="0.3" />
        <rect x="13" y="6.7" width="1" height="2.6" rx="0.3" />
      </g>
    </Icon>
  );
}

/** Each database system with an icon, by a plain id, with the name to show. */
export const DATABASES: Record<string, { name: string; icon: Component<IconProps> }> = {
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

/** The name of a database system by its id in `DATABASES`, such as
 * `PostgreSQL`, or the id for a system it does not know. */
export const databaseName = (id: string) => DATABASES[id]?.name ?? id;

/** The icon of a database system by its id in `DATABASES`, titled with its
 * name, or the plain database icon, in the database color, for a system it
 * does not know. */
export function DatabaseSystemIcon(props: IconProps & { system: string }) {
  const known = () => DATABASES[props.system];
  return (
    <Dynamic
      component={known()?.icon ?? DatabaseIcon}
      size={props.size}
      class={known() ? props.class : `text-database ${props.class ?? ""}`}
      title={props.title ?? databaseName(props.system)}
    />
  );
}
