---
name: sql
description: How otelo writes SQL. A column has the name of the Rust field that holds it, a value is a bound parameter, and a renamed column raises the schema version. Invoke before creating or editing any .sql file under crates/, before writing SQL in a .rs file, and before reviewing either.
---

# Writing the SQL

The schema is `schema.sql` and `rollup.sql` in `crates/storage-sqlite/src/`. The statements
are strings in the Rust sources of that crate.

## Names

- A user types these names into `otelo sql`. Choose one as you choose a CLI flag.
- A table is a plural noun: `resources`, `attribute_keys`. A column is singular. Both are
  snake_case and spelled in full: `attribute`, not `attr`.
- A column has the name of the Rust field that holds it: `logs.logged_at` is
  `Log::logged_at`, and `attribute_keys.key_group` is a `KeyGroup`.
- A column that holds an instant ends in `_at`: `logged_at`, `started_at`, `start_at`.
  Never `ts` or `time`.
- A quantity says its unit: `duration_ns`.
- A flag says what is true when it is 1: `has_more_values`, not `many_values`.
- A foreign key is the singular of its table and `_id`: `resource_id`, `series_id`.
- An index is its table and its columns: `logs_logged_at`, `spans_trace_id`. The index of
  an attribute is `<table>_attribute_<hash>`.

## The schema files

- Uppercase keywords and types, lowercase names, two spaces of indent, one column per line.
- An index comes right after its table.
- Every `CREATE` has `IF NOT EXISTS`, because the file runs each time a database opens.
- A column is `NOT NULL` unless a missing value means something: the `parent_span_id` of a
  root span.
- A table with a primary key other than `INTEGER PRIMARY KEY` is `WITHOUT ROWID`.
- No comments. What a column holds goes in the `telemetry` doc, next to the diagram of the
  schema.
- The only comment that earns its place is a reason the SQL cannot show. Write it as a short
  `--` at the statement it explains.

## SQL in Rust

- Uppercase keywords, lowercase functions: `count(*)`, `json_extract`.
- A statement that does not fit on one line breaks before `FROM`, `JOIN`, `WHERE`,
  `GROUP BY`, and `ORDER BY`.
- A table alias is the singular of the table: `spans span`, `resources resource`. A row of
  `minutes` or `hours` is a `summary`. A table the query reads twice says which one:
  `matching_span`. `series` takes no alias.
- A value is a bound parameter, never text in the statement. A query built from parts
  names its parameters, `:since`. A fixed statement numbers them, `?1`.
- Rust formats only names and numbers of its own into a statement: a table, a column, a
  limit, a row id it read.
- The number of an OpenTelemetry enum comes from its type: `SpanKind::Server.number()`,
  never `2`.
- The path of an attribute comes from `attribute_json_path`, so the expression is the one
  its index has and SQLite uses the index.
- The schema of a day file goes in double quotes, `"2026-09-30".spans`. A string goes in
  single quotes.
- `GROUP BY` and `ORDER BY` name their columns, never `1, 2`. To sort by an aggregate,
  name it with `AS`.

## Changing a name on disk

- A day file and the rollup file keep their `SCHEMA_VERSION` in `PRAGMA user_version`.
- A new table or index needs no new version. `IF NOT EXISTS` adds it to an old file.
- A renamed, removed, or retyped column raises the version. There is no migration: the
  writer sets a file of another version aside, and its rows leave every query.
- So a rename costs the users their history and changes what they type into `otelo sql`.
  Rename a name that is wrong, not one that could be nicer, and say so in the commit
  message.
- The same commit changes the `telemetry` doc, the tests, and the SQL in
  `packages/app/tests`.
