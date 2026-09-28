use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::{cut, hex, statement_span};
use crate::Reader;

/// The longest string or blob a query can make, so that one query cannot take
/// the memory of the server.
const MAX_LENGTH: i32 = 16 * 1024 * 1024;

/// The pragmas a query can read: the ones that describe the schema.
const PRAGMAS: [&str; 6] = [
    "database_list",
    "index_info",
    "index_list",
    "table_info",
    "table_list",
    "table_xinfo",
];

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct SqlResult {
    pub columns: Vec<String>,
    /// Each row has one value per column. A blob is its hex digits.
    #[schema(value_type = Vec<Vec<Object>>)]
    pub rows: Vec<Vec<Value>>,
    /// The query returned more rows than the limit let through.
    pub truncated: bool,
}

impl Reader {
    /// Runs one read-only `SELECT` and returns its first `limit` rows. The
    /// query can read the tables and views of the reader and describe the
    /// schema with a pragma. It cannot attach a file or change anything.
    ///
    /// # Errors
    ///
    /// When the SQL is not one statement, when it does more than read, or when
    /// it fails.
    pub fn sql(&self, sql: &str, limit: usize) -> anyhow::Result<SqlResult> {
        let conn = self.conn();
        conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_LENGTH)?;
        conn.authorizer(Some(authorize))?;
        let result = run(self, sql, limit);
        conn.authorizer(None::<fn(AuthContext<'_>) -> Authorization>)?;
        result
    }
}

fn run(reader: &Reader, sql: &str, limit: usize) -> anyhow::Result<SqlResult> {
    let span = statement_span(sql);
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
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(n) => n.into(),
                    ValueRef::Real(x) => x.into(),
                    ValueRef::Text(text) => String::from_utf8_lossy(text).into(),
                    ValueRef::Blob(blob) => hex(blob).into(),
                })
            })
            .collect::<anyhow::Result<_>>()?;
        rows.push(values);
    }
    span.record(
        "db.response.returned_rows",
        i64::try_from(rows.len()).unwrap_or(i64::MAX),
    );
    let truncated = cut(&mut rows, limit);
    Ok(SqlResult {
        columns,
        rows,
        truncated,
    })
}

fn authorize(context: AuthContext<'_>) -> Authorization {
    match context.action {
        AuthAction::Select
        | AuthAction::Read { .. }
        | AuthAction::Function { .. }
        | AuthAction::Recursive => Authorization::Allow,
        AuthAction::Pragma { pragma_name, .. }
            if PRAGMAS.contains(&pragma_name.to_ascii_lowercase().as_str()) =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }
}
