use otelo_api::{Completions, IndexList};
use otelo_query::Signal;
use otelo_storage::query::AttributeKeys;

use super::client::{Client, OutputFormat, escape_path_segment, print_json};
use super::table::Table;

#[derive(clap::Args)]
pub struct AttributesArgs {
    /// logs, spans, or metrics
    signal: Signal,

    #[command(flatten)]
    client: Client,
}

pub fn print_attributes(args: &AttributesArgs) -> anyhow::Result<()> {
    let attributes: AttributeKeys = args.client.get(
        "/api/attributes",
        &[("signal", Some(args.signal.to_string()))],
    )?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            let mut table = Table::new(&["FIELD", "TYPE", "COUNT", "INDEXED"]);
            let rows = attributes
                .record
                .iter()
                .map(|attribute| {
                    (
                        otelo_query::Field::Attribute(attribute.key.clone()),
                        attribute,
                    )
                })
                .chain(attributes.resource.iter().map(|attribute| {
                    (
                        otelo_query::Field::Resource(attribute.key.clone()),
                        attribute,
                    )
                }));
            for (field, attribute) in rows {
                table.add_row(vec![
                    field.to_string(),
                    attribute.value_type.to_string(),
                    attribute.count.to_string(),
                    if attribute.indexed { "yes" } else { "" }.into(),
                ]);
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(&attributes)?,
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

pub fn print_completions(args: &CompleteArgs) -> anyhow::Result<()> {
    let completions: Completions = args.client.get(
        "/api/complete",
        &[
            ("signal", Some(args.signal.to_string())),
            ("q", Some(args.query.clone())),
            ("cursor", args.cursor.map(|cursor| cursor.to_string())),
        ],
    )?;
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            let mut table = Table::new(&["SUGGESTION", "KIND", "DETAIL"]);
            for suggestion in &completions.suggestions {
                table.add_row(vec![
                    suggestion.text.clone(),
                    suggestion.kind.to_string(),
                    suggestion.detail.clone().unwrap_or_default(),
                ]);
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(&completions)?,
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

pub fn change_and_print_indexes(args: &IndexArgs) -> anyhow::Result<()> {
    let build_index_path =
        |signal: &Signal, key: &str| format!("/api/indexes/{signal}/{}", escape_path_segment(key));
    let index_list: IndexList = match &args.command {
        IndexCommand::List => args.client.get("/api/indexes", &[])?,
        IndexCommand::Add { signal, key } => args.client.put(&build_index_path(signal, key))?,
        IndexCommand::Remove { signal, key } => {
            args.client.delete(&build_index_path(signal, key))?
        }
    };
    match args.client.choose_output_format() {
        OutputFormat::Table => {
            let mut table = Table::new(&["SIGNAL", "KEY"]);
            for index in &index_list.indexes {
                table.add_row(vec![index.signal.to_string(), index.key.clone()]);
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(&index_list)?,
    }
    Ok(())
}
