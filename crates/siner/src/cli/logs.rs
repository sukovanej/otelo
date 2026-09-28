//! `siner logs`: log lines, or their groups by message template.

use siner_query::Signal;
use siner_telemetry::query::{GROUP_SCAN_LIMIT, LogGroups, Logs};

use super::client::{Client, note_cut, print_json};
use super::table::{self, Table};
use super::{QUERY_HELP, Range, joined, note_unindexed};

#[derive(clap::Args)]
pub struct LogsArgs {
    #[arg(help = QUERY_HELP)]
    query: Vec<String>,

    /// Print the lines, not the groups by message template
    #[arg(long)]
    raw: bool,

    #[command(flatten)]
    range: Range,

    #[command(flatten)]
    client: Client,
}

/// Runs `siner logs`.
///
/// # Errors
///
/// When the daemon cannot be reached, or answers with an error.
pub fn logs(args: &LogsArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("q", joined(&args.query)));
    let narrow = "narrow them with the query or --since, or raise --limit";
    if args.raw {
        let logs: Logs = args.client.get("/api/logs", &params)?;
        if args.client.wants_table() {
            let mut table = Table::new(&["TIME (UTC)", "SERVICE", "LEVEL", "TRACE", "BODY"]);
            for line in &logs.logs {
                table.row(vec![
                    table::time(line.time),
                    line.service.clone(),
                    line.level.clone(),
                    line.trace_id.clone().unwrap_or_else(|| "-".into()),
                    line.body.clone(),
                ]);
            }
            table.print()?;
        } else {
            print_json(&logs)?;
        }
        note_cut(logs.truncated, &format!("More lines match; {narrow}."));
        note_unindexed(Signal::Logs, &logs.unindexed);
        return Ok(());
    }
    let groups: LogGroups = args.client.get("/api/logs/groups", &params)?;
    if args.client.wants_table() {
        let mut table = Table::new(&["COUNT", "LEVEL", "SERVICE", "LAST (UTC)", "TEMPLATE"]);
        for group in &groups.groups {
            table.row(vec![
                group.count.to_string(),
                group.level.clone(),
                group.services.join(","),
                table::time(group.last),
                group.template.clone(),
            ]);
            if let Some(sample) = group.samples.first().filter(|s| **s != group.template) {
                table.under(&format!("e.g. {sample}"));
            }
        }
        table.print()?;
    } else {
        print_json(&groups)?;
    }
    note_cut(groups.truncated, &format!("More groups exist; {narrow}."));
    note_cut(
        groups.partial,
        &format!(
            "The groups count only the newest {GROUP_SCAN_LIMIT} lines; narrow the query or --since."
        ),
    );
    note_unindexed(Signal::Logs, &groups.unindexed);
    Ok(())
}
