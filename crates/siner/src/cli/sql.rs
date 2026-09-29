use std::io::{self, Read};

use siner_api::SqlRequest;
use siner_storage::query::{SqlResult, SqlValue};

use super::Range;
use super::client::{Client, note_cut, print_json};
use super::table::Table;

#[derive(clap::Args)]
pub struct SqlArgs {
    /// One SELECT, or - to read it from stdin. It reads the views resources,
    /// logs, spans, series, and points, which join the day files of the range
    /// with a day column in front, and the attribute catalog
    query: String,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

pub fn sql(args: &SqlArgs) -> anyhow::Result<()> {
    let sql = if args.query == "-" {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text)?;
        text
    } else {
        args.query.clone()
    };
    let request = SqlRequest {
        sql,
        since: args.range.since.clone(),
        until: args.range.until.clone(),
        limit: args.range.limit,
    };
    let result: SqlResult = args.client.post("/api/sql", &request)?;
    if args.client.wants_table() {
        let columns: Vec<&str> = result.columns().iter().map(String::as_str).collect();
        let mut table = Table::new(&columns);
        for row in result.rows() {
            table.row(row.iter().map(|value| match value {
                SqlValue::Null => String::new(),
                other => other.to_string(),
            }));
        }
        table.print()?;
    } else {
        print_json(&result)?;
    }
    note_cut(
        result.truncated(),
        "The query returned more rows; narrow it with a WHERE, or raise --limit.",
    );
    Ok(())
}
