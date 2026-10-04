use std::io::Read as _;
use std::path::PathBuf;

use anyhow::Context;
use otelo_state::{Dashboard, DashboardDefinition, DashboardId, DashboardList};

use super::client::{Client, OutputFormat, print_json};
use super::table::{Table, format_utc_time};

#[derive(clap::Args)]
pub struct DashboardArgs {
    #[command(subcommand)]
    command: DashboardCommand,

    #[command(flatten)]
    client: Client,
}

#[derive(clap::Subcommand)]
enum DashboardCommand {
    /// List the dashboards, the most recently changed first
    List,
    /// Print a dashboard as the JSON that create and replace take
    Show {
        /// The ID of the dashboard
        id: DashboardId,
    },
    /// Save a new dashboard from a JSON file with its name, description, and widgets
    Create {
        /// The JSON file, or - for stdin
        file: PathBuf,
    },
    /// Replace what a dashboard shows with a JSON file
    Replace {
        /// The ID of the dashboard
        id: DashboardId,
        /// The JSON file, or - for stdin
        file: PathBuf,
    },
    /// Delete a dashboard
    Delete {
        /// The ID of the dashboard
        id: DashboardId,
    },
}

pub fn change_and_print_dashboards(args: &DashboardArgs) -> anyhow::Result<()> {
    match &args.command {
        DashboardCommand::List => {
            print_dashboard_list(&args.client, &args.client.get("/api/dashboards", &[])?)
        }
        DashboardCommand::Show { id } => {
            let dashboard: Dashboard = args.client.get(&format!("/api/dashboards/{id}"), &[])?;
            print_json(&dashboard.definition)
        }
        DashboardCommand::Create { file } => {
            let dashboard: Dashboard = args
                .client
                .post_json("/api/dashboards", &read_definition(file)?)?;
            print_saved_dashboard(&args.client, &dashboard)
        }
        DashboardCommand::Replace { id, file } => {
            let dashboard: Dashboard = args
                .client
                .put_json(&format!("/api/dashboards/{id}"), &read_definition(file)?)?;
            print_saved_dashboard(&args.client, &dashboard)
        }
        DashboardCommand::Delete { id } => print_dashboard_list(
            &args.client,
            &args.client.delete(&format!("/api/dashboards/{id}"))?,
        ),
    }
}

fn read_definition(file: &PathBuf) -> anyhow::Result<DashboardDefinition> {
    let text = if file.as_os_str() == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .context("read the dashboard from stdin")?;
        text
    } else {
        std::fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?
    };
    serde_json::from_str(&text).context("read the JSON of the dashboard")
}

fn print_dashboard_list(client: &Client, list: &DashboardList) -> anyhow::Result<()> {
    match client.choose_output_format() {
        OutputFormat::Table => {
            let mut table = Table::new(&["ID", "NAME", "WIDGETS", "UPDATED", "DESCRIPTION"]);
            for dashboard in &list.dashboards {
                table.add_row(vec![
                    dashboard.id.to_string(),
                    dashboard.name.clone(),
                    dashboard.widget_count.to_string(),
                    format_utc_time(dashboard.updated_at),
                    dashboard.description.clone(),
                ]);
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(list)?,
    }
    Ok(())
}

fn print_saved_dashboard(client: &Client, dashboard: &Dashboard) -> anyhow::Result<()> {
    match client.choose_output_format() {
        OutputFormat::Table => println!(
            "Saved dashboard {} with {} widgets: {}",
            dashboard.id,
            dashboard.definition.widgets().len(),
            dashboard.definition.name()
        ),
        OutputFormat::Json => print_json(dashboard)?,
    }
    Ok(())
}
