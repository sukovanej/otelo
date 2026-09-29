use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;
use siner_storage::query::{SqlResult, SqlValue};

use super::{hex_digits, new_statement_span, truncate_to_limit};
use crate::Reader;

// One query must not take the memory of the server.
const MAX_VALUE_BYTES: i32 = 16 * 1024 * 1024;

const SCHEMA_PRAGMAS: [&str; 6] = [
    "database_list",
    "index_info",
    "index_list",
    "table_info",
    "table_list",
    "table_xinfo",
];

fn run_select(reader: &Reader, sql: &str, limit: usize) -> anyhow::Result<SqlResult> {
    let span = new_statement_span(sql);
    let _entered = span.enter();
    let mut stmt = reader.conn().prepare(sql)?;
    let columns: Vec<String> = stmt.column_names().into_iter().map(Into::into).collect();
    let mut rows = Vec::new();
    let mut query = stmt.query([])?;
    while let Some(row) = query.next()? {
        if rows.len() > limit {
            break;
        }
        let values = (0..columns.len())
            .map(|i| {
                Ok(match row.get_ref(i)? {
                    ValueRef::Null => SqlValue::Null,
                    ValueRef::Integer(n) => SqlValue::Integer(n),
                    ValueRef::Real(x) => SqlValue::Real(x),
                    ValueRef::Text(text) => SqlValue::Text(String::from_utf8_lossy(text).into()),
                    ValueRef::Blob(blob) => SqlValue::Text(hex_digits(blob)),
                })
            })
            .collect::<anyhow::Result<_>>()?;
        rows.push(values);
    }
    span.record(
        "db.response.returned_rows",
        i64::try_from(rows.len()).unwrap_or(i64::MAX),
    );
    let truncated = truncate_to_limit(&mut rows, limit);
    SqlResult::new(columns, rows, truncated)
}

fn authorize_read_only(context: AuthContext<'_>) -> Authorization {
    match context.action {
        AuthAction::Select
        | AuthAction::Read { .. }
        | AuthAction::Function { .. }
        | AuthAction::Recursive => Authorization::Allow,
        AuthAction::Pragma { pragma_name, .. }
            if SCHEMA_PRAGMAS.contains(&pragma_name.to_ascii_lowercase().as_str()) =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }
}

pub(super) fn run_user_sql(reader: &Reader, sql: &str, limit: usize) -> anyhow::Result<SqlResult> {
    let conn = reader.conn();
    conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_VALUE_BYTES)?;
    conn.authorizer(Some(authorize_read_only))?;
    let result = run_select(reader, sql, limit);
    conn.authorizer(None::<fn(AuthContext<'_>) -> Authorization>)?;
    result
}
