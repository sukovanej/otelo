use otelo_storage::query::{SqlResult, SqlValue};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;

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
    let statement_span = new_statement_span(sql);
    let _entered = statement_span.enter();
    let mut statement = reader.connection().prepare(sql)?;
    let columns: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(Into::into)
        .collect();
    let mut rows = Vec::new();
    let mut result_rows = statement.query([])?;
    while let Some(row) = result_rows.next()? {
        if rows.len() > limit {
            break;
        }
        let values = (0..columns.len())
            .map(|column_index| {
                Ok(match row.get_ref(column_index)? {
                    ValueRef::Null => SqlValue::Null,
                    ValueRef::Integer(integer) => SqlValue::Integer(integer),
                    ValueRef::Real(real) => SqlValue::Real(real),
                    ValueRef::Text(text) => SqlValue::Text(String::from_utf8_lossy(text).into()),
                    ValueRef::Blob(blob) => SqlValue::Text(hex_digits(blob)),
                })
            })
            .collect::<anyhow::Result<_>>()?;
        rows.push(values);
    }
    statement_span.record(
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
    let connection = reader.connection();
    connection.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_VALUE_BYTES)?;
    connection.authorizer(Some(authorize_read_only))?;
    let result = run_select(reader, sql, limit);
    connection.authorizer(None::<fn(AuthContext<'_>) -> Authorization>)?;
    result
}
