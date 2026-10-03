use otelo_indexed_storage::query::{LogGroups, Logs, MAX_GROUPED_LOG_LINES};
use otelo_query::Signal;

use super::client::{Client, OutputFormat, note_truncation, print_json};
use super::table::{self, Table};
use super::{QUERY_HELP, RangeArgs, join_query_words, note_unindexed_keys};

#[derive(clap::Args)]
pub struct LogsArgs {
    #[arg(help = QUERY_HELP)]
    query: Vec<String>,

    /// Print the lines, not the groups by message template
    #[arg(long)]
    raw: bool,

    #[command(flatten)]
    range: RangeArgs,

    #[command(flatten)]
    client: Client,
}

pub fn print_logs(args: &LogsArgs) -> anyhow::Result<()> {
    let mut params = args.range.to_query_params();
    params.push(("q", join_query_words(&args.query)));
    let narrowing_advice = "narrow them with the query or --since, or raise --limit";
    if args.raw {
        let answer: Logs = args.client.get("/api/logs", &params)?;
        match args.client.choose_output_format() {
            OutputFormat::Table => {
                let mut table = Table::new(&["TIME (UTC)", "SERVICE", "LEVEL", "TRACE", "BODY"]);
                for line in &answer.logs {
                    table.add_row(vec![
                        table::format_utc_time(line.logged_at),
                        line.service.clone(),
                        line.severity.level().into(),
                        line.trace_id
                            .map_or_else(|| "-".into(), |id| id.to_string()),
                        line.body.clone(),
                    ]);
                }
                table.print()?;
            }
            OutputFormat::Json => print_json(&answer)?,
        }
        note_truncation(
            answer.truncated,
            &format!("More lines match; {narrowing_advice}."),
        );
        note_unindexed_keys(Signal::Logs, &answer.unindexed);
        return Ok(());
    }
    let answer: LogGroups = args.client.get("/api/logs/groups", &params)?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            let mut table = Table::new(&["COUNT", "LEVEL", "SERVICE", "LAST (UTC)", "TEMPLATE"]);
            for group in &answer.groups {
                table.add_row(vec![
                    group.count.to_string(),
                    group.severity.level().into(),
                    group.services.join(","),
                    table::format_utc_time(group.last_at),
                    group.template.clone(),
                ]);
                if let Some(sample) = group
                    .samples
                    .first()
                    .filter(|sample| **sample != group.template)
                {
                    table.add_line_under_last_row(&format!("e.g. {sample}"));
                }
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(&answer)?,
    }
    note_truncation(
        answer.truncated,
        &format!("More groups exist; {narrowing_advice}."),
    );
    note_truncation(
        answer.partial,
        &format!(
            "The groups count only the newest {MAX_GROUPED_LOG_LINES} lines; narrow the query or --since."
        ),
    );
    note_unindexed_keys(Signal::Logs, &answer.unindexed);
    Ok(())
}
