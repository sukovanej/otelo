---
name: rust
description: How otelo writes Rust. Invalid states are unrepresentable, names say what a thing is, and there are no comments. Invoke before creating or editing any .rs file under crates/, and before reviewing Rust.
---

# Writing the Rust

## Invalid states are unrepresentable

The state that cannot happen has no shape to write it in.

- `TimeRange` has private fields and a constructor that checks the start is before the end.
  A backwards range cannot be written.
- `IndexedAttribute` takes an `IndexedSignal`, which has no metrics variant, because
  metrics have no attribute indexes.
- `TraceSpan` has no `error` flag. `status` already says whether the span failed.
- Three states are three enum variants, never two `Option` fields.
- No field another field implies.
- A newtype per id: `TraceId([u8; 16])`, `SpanId([u8; 8])`. Never a bare `i64`, byte array,
  or code number across a boundary: `SpanKind`, `SpanStatus`, and `Severity` wrap the
  OpenTelemetry numbers.

## Names

- A name says what the thing is, in full. A lone verb almost never does: `open_range`,
  not `read`; `classify_query_error`, not `error`.
- Functions take a verb *and* what it acts on: `apply_indexes_to_day_file`,
  `add_point_to_bucket`, `resolve_range`.
- The verb is the domain action, never the machinery: `spawn_writer`, not `start`.
- A qualifier is not a description. `oldest` says which, not what: `oldest_retained_at`.
- Constants name what they hold: `MAX_ATTACHED_DAYS`, not `MAX_DAYS`.
- A field holding an instant ends in one: `logged_at`, `started_at`, not `time` or `ts`.
- A binding is named for its role, not its type: an `IndexedAttribute` is an `attribute`,
  and `key.key` means the name is wrong.

## No comments

- No `//`, no `///`, no `//!`.
- A comment means the code is not self-documenting. Rename or restructure until it is.
- The only one that earns its place is intent the code cannot express: why, a constraint,
  an external fact. Write it as a short `//` at the code it explains.
- Doc comments that are output stay: on items that derive utoipa's `ToSchema` or
  `IntoParams` and their fields, on `#[utoipa::path]` handlers, and on clap-derived items
  and their fields. They are the OpenAPI spec and the CLI help.
- Delete the ones you meet that do not.
