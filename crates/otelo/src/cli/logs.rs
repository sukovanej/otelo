use otelo_query::Signal;
use otelo_storage::query::{GROUP_SCAN_LIMIT, LogGroups, Logs};

use super::client::{Client, note_cut, print_json};
use super::table::{self, Table};
use super::{QUERY_HELP, Range, join_query_words, note_unindexed};

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

pub fn logs(args: &LogsArgs) -> anyhow::Result<()> {
    let mut params = args.range.params();
    params.push(("q", join_query_words(&args.query)));
    let narrow = "narrow them with the query or --since, or raise --limit";
    if args.raw {
        let logs: Logs = args.client.get("/api/logs", &params)?;
        if args.client.wants_table() {
            let mut table = Table::new(&["TIME (UTC)", "SERVICE", "LEVEL", "TRACE", "BODY"]);
            for line in &logs.logs {
                table.row(vec![
                    table::format_utc_time(line.logged_at),
                    line.service.clone(),
                    line.severity.level().into(),
                    line.trace_id
                        .map_or_else(|| "-".into(), |id| id.to_string()),
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
                group.severity.level().into(),
                group.services.join(","),
                table::format_utc_time(group.last_at),
                group.template.clone(),
            ]);
            if let Some(sample) = group.samples.first().filter(|s| **s != group.template) {
                table.add_line_under_last_row(&format!("e.g. {sample}"));
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
