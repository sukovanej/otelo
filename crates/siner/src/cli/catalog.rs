use siner_api::{Completions, IndexList};
use siner_query::Signal;
use siner_storage::query::AttributeKeys;

use super::client::{Client, escape_path_segment, print_json};
use super::table::Table;

#[derive(clap::Args)]
pub struct AttributesArgs {
    /// logs, spans, or metrics
    signal: Signal,

    #[command(flatten)]
    client: Client,
}

pub fn attributes(args: &AttributesArgs) -> anyhow::Result<()> {
    let attributes: AttributeKeys = args.client.get(
        "/api/attributes",
        &[("signal", Some(args.signal.to_string()))],
    )?;
    if args.client.wants_table() {
        let mut table = Table::new(&["FIELD", "TYPE", "COUNT", "INDEXED"]);
        let rows = attributes
            .record
            .iter()
            .map(|a| (siner_query::Field::Attribute(a.key.clone()), a))
            .chain(
                attributes
                    .resource
                    .iter()
                    .map(|a| (siner_query::Field::Resource(a.key.clone()), a)),
            );
        for (field, attribute) in rows {
            table.row(vec![
                field.to_string(),
                attribute.kind.clone(),
                attribute.count.to_string(),
                if attribute.indexed { "yes" } else { "" }.into(),
            ]);
        }
        table.print()?;
    } else {
        print_json(&attributes)?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct CompleteArgs {
    /// logs, spans, or metrics
    signal: Signal,

    /// The query as typed so far
    #[arg(default_value = "")]
    query: String,

    /// The position of the cursor in the query, in characters [default: the
    /// end]
    #[arg(long)]
    cursor: Option<usize>,

    #[command(flatten)]
    client: Client,
}

pub fn complete(args: &CompleteArgs) -> anyhow::Result<()> {
    let completions: Completions = args.client.get(
        "/api/complete",
        &[
            ("signal", Some(args.signal.to_string())),
            ("q", Some(args.query.clone())),
            ("cursor", args.cursor.map(|n| n.to_string())),
        ],
    )?;
    if args.client.wants_table() {
        let mut table = Table::new(&["SUGGESTION", "KIND", "DETAIL"]);
        for suggestion in &completions.suggestions {
            table.row(vec![
                suggestion.text.clone(),
                suggestion.kind.to_string(),
                suggestion.detail.clone().unwrap_or_default(),
            ]);
        }
        table.print()?;
    } else {
        print_json(&completions)?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct IndexArgs {
    #[command(subcommand)]
    command: IndexCommand,

    #[command(flatten)]
    client: Client,
}

#[derive(clap::Subcommand)]
enum IndexCommand {
    /// List the indexed attributes
    List,
    /// Index an attribute of the logs or the spans in every day file
    Add {
        /// logs or spans
        signal: Signal,
        /// The attribute key, such as user.id
        key: String,
    },
    /// Drop the index of an attribute
    Remove {
        /// logs or spans
        signal: Signal,
        /// The attribute key
        key: String,
    },
}

pub fn index(args: &IndexArgs) -> anyhow::Result<()> {
    let path =
        |signal: &Signal, key: &str| format!("/api/indexes/{signal}/{}", escape_path_segment(key));
    let list: IndexList = match &args.command {
        IndexCommand::List => args.client.get("/api/indexes", &[])?,
        IndexCommand::Add { signal, key } => args.client.put(&path(signal, key))?,
        IndexCommand::Remove { signal, key } => args.client.delete(&path(signal, key))?,
    };
    if args.client.wants_table() {
        let mut table = Table::new(&["SIGNAL", "KEY"]);
        for index in &list.indexes {
            table.row(vec![index.signal.to_string(), index.key.clone()]);
        }
        table.print()?;
    } else {
        print_json(&list)?;
    }
    Ok(())
}
