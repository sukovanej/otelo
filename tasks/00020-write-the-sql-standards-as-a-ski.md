---
status: done
created: 2026-09-30T15:35:51Z
pull_requests:
- https://github.com/sukovanej/otelo/pull/22
---
# Write the SQL standards as a skill

The `rust` and `frontend-typescript` skills say how otelo writes Rust and TypeScript. Nothing says how it writes SQL, so the schema and the queries follow no written rule. Write a `sql` skill in `.claude/skills/` that does, then bring the SQL in line with it.

The audit of the codebase against the `rust` skill found these names on disk that break its rules, and left them alone because SQL had no rules of its own:

- `logs.ts`, `spans.start_ts`, `points.ts`, and the index `spans_start_ts`. The Rust fields for the same instants are `logged_at` and `started_at`.
- `attribute_keys.type`, which the Rust code reads into a field called `kind`.
- The `signal` column of the catalog tables, which holds two values that are not signals.
- The `attr_` prefix of the attribute indexes. The loop that drops an index finds it by this prefix, so a rename orphans the indexes that exist.
- `many_values`, which is a column, an API field, and a frontend name.
- Seven comments in `crates/storage-sqlite/src/schema.sql`.

The skill has to decide at least:

- how a table, a column, and an index are named, and whether a column that holds an instant ends in `_at` as the Rust field does
- whether comments are allowed in `schema.sql`
- how the SQL in the Rust sources is written: keyword case, layout, where a fragment is built from a Rust type such as `SpanKind` and not from a literal number
- how a name on disk changes. The day files have no migration, and the `sql` query shows the names to users, so a rename changes what they type.

## Comments

### 2026-09-30T16:35:19Z by Milan Suk via claude-code

> Left as they are: telemetry_indexes.signal in state.sqlite (real signals, and the file has no schema version) and cursors.rolled_until. The rollup file is now set aside at another version, which the task did not ask for; a rename of minutes.start needed it.
