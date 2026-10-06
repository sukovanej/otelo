---
name: sql
description: How otelo writes SQL. A column has the name of the Rust field that holds it, a value is a bound parameter, and the schema changes freely while otelo is in development. Invoke before creating or editing any .sql file under crates/, before writing SQL in a .rs file, and before reviewing either.
metadata:
  internal: true
---

# Writing the SQL

Two crates write SQL, each for its own file:

- `otelo-indexed-storage-sqlite` for `telemetry.sqlite`. Its schema is `schema.sql` in
  `crates/indexed-storage-sqlite/src/`.
- `otelo-state` for `state.sqlite`. Its schema is `schema.sql` in `crates/state/src/`.

The statements are strings in the Rust sources of the crate that owns the file.

## Names

- The names are private to the crate that owns the file. No command runs SQL that a user
  wrote, and no other crate reads the tables. Keep it so.
- A table is a plural noun: `resources`, `attribute_key_counts`. A column is singular. Both are
  snake_case and spelled in full: `attribute`, not `attr`.
- A name says what the table or the column holds without its comment, and takes the
  OpenTelemetry name where OpenTelemetry has one: `severity_number`, `aggregation_temporality`.
- A column has the name of the Rust field that holds it: `logs.logged_at` is
  `Log::logged_at`, and `attribute_key_counts.attribute_owner` is an `AttributeOwner`.
- A column that holds an instant ends in `_at`: `logged_at`, `started_at`, `start_at`.
  Never `ts` or `time`.
- A quantity says its unit: `duration_ns`.
- A flag says what is true when it is 1: `has_more_values_than_listed`, not `many_values`.
- A foreign key is the singular of its table and `_id`: `resource_id`, `metric_series_id`.
- An index is its table and its columns: `logs_logged_at`, `spans_trace_id`. The index of
  an attribute is `<table>_attribute_<hash>`.

## The schema files

- Uppercase keywords and types, lowercase names, two spaces of indent, one column per line.
- An index comes right after its table.
- Every `CREATE` has `IF NOT EXISTS`, because the file runs each time a database opens.
- A column is `NOT NULL` unless a missing value means something: the `parent_span_id` of a
  root span.
- A table with a primary key other than `INTEGER PRIMARY KEY` is `WITHOUT ROWID`.
- A column gets a comment when its definition leaves out a fact a reader needs. Write it
  as `--` lines above the column:
  - the values of an enum, which SQLite cannot declare:
    `-- gauge, updown, counter, or histogram.`
  - the unit or the encoding of a value: `-- Unix nanoseconds.`, `-- A JSON object.`
  - what `NULL` means: `-- NULL for a root span.`
  - what a hash or a count covers.
- No comment repeats the name or the type of the column.
- A table or a statement gets a comment only for a reason the SQL cannot show.

## SQL in Rust

- Uppercase keywords, lowercase functions: `count(*)`, `json_extract`.
- A statement that does not fit on one line breaks before `FROM`, `JOIN`, `WHERE`,
  `GROUP BY`, and `ORDER BY`.
- A table alias is the singular of the table: `spans span`, `resources resource`. A row of
  `metric_minute_summaries` or `metric_hour_summaries` is a `summary`. A table the query reads
  twice says which one: `matching_span`. `metric_series` takes no alias.
- A value is a bound parameter, never text in the statement. A query built from parts
  names its parameters, `:since`. A fixed statement numbers them, `?1`.
- A statement has the same text for every call of its query. otelo records the text as the
  `db.query.text` of a span, and the services view groups spans by it, so a value in the
  text makes a group of each call.
- Rust formats only names and constants of its own into a statement: a table, a column,
  `MAX_CATALOG_ROWS`, `SpanStatus::Error.number()`. A limit, a row id it read, and the
  level or kind a query names are parameters.
- A list of values is one parameter: a JSON array that the statement reads with
  `json_each`, `rowid IN (SELECT value FROM json_each(:rowids))`. A blob goes in the array
  as hex and comes out with `unhex(value)`.
- The number of an OpenTelemetry enum comes from its type: `SpanKind::Server.number()`,
  never `2`.
- The path of an attribute comes from `attribute_json_path`, so the expression is the one
  its index has and SQLite uses the index.
- A string goes in single quotes.
- `GROUP BY` and `ORDER BY` name their columns, never `1, 2`. To sort by an aggregate,
  name it with `AS`.

## Changing the schema

- otelo is in development and the tables are still being designed. Change a table, a
  column, or an index as soon as a better design shows up.
- No code migrates a file of an older schema.
- `state.sqlite` carries no schema version. After a change of its schema, delete the state
  files written before it, such as `target/dev/state.sqlite`.
- The journal keeps the telemetry, and `otelo reindex` builds `telemetry.sqlite` from it
  again. A change of its `schema.sql` bumps `STORAGE_VERSION` in `version.rs` and sets
  `SCHEMA_HASH_AT_STORAGE_VERSION` to the hash the failing test prints. A change of the
  mapping that changes what is stored bumps `STORAGE_VERSION` too.
- `otelo serve` does not start on a file of another version. Run `otelo reindex` with the
  daemon stopped, such as `cargo run -q -- reindex --data target/dev`.
- The same commit changes the `telemetry` doc and the tests.
