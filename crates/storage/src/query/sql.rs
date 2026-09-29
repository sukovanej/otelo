use anyhow::bail;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "UncheckedSqlResult")]
pub struct SqlResult {
    columns: Vec<String>,
    /// Each row has one value per column.
    rows: Vec<Vec<SqlValue>>,
    /// The query returned more rows than the limit let through.
    truncated: bool,
}

#[derive(Deserialize)]
struct UncheckedSqlResult {
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
    truncated: bool,
}

impl SqlResult {
    pub fn new(
        columns: Vec<String>,
        rows: Vec<Vec<SqlValue>>,
        truncated: bool,
    ) -> anyhow::Result<Self> {
        if let Some(row) = rows.iter().find(|row| row.len() != columns.len()) {
            bail!(
                "a row has {} values for {} columns",
                row.len(),
                columns.len()
            );
        }
        Ok(Self {
            columns,
            rows,
            truncated,
        })
    }

    #[must_use]
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    #[must_use]
    pub fn rows(&self) -> &[Vec<SqlValue>] {
        &self.rows
    }

    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

impl TryFrom<UncheckedSqlResult> for SqlResult {
    type Error = anyhow::Error;

    fn try_from(unchecked: UncheckedSqlResult) -> anyhow::Result<Self> {
        Self::new(unchecked.columns, unchecked.rows, unchecked.truncated)
    }
}

/// A value of a SQLite column. A blob is its hex digits, as text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

impl std::fmt::Display for SqlValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => f.write_str("NULL"),
            Self::Integer(n) => write!(f, "{n}"),
            Self::Real(x) => write!(f, "{x}"),
            Self::Text(text) => f.write_str(text),
        }
    }
}
