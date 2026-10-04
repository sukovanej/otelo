use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use super::client::{OutputArgs, OutputFormat, print_json};
use super::login::{StoredPassword, delete_stored_password, read_stored_password};
use super::table::Table;

#[derive(clap::Args)]
pub struct RemoteArgs {
    #[command(subcommand)]
    command: RemoteCommand,

    #[command(flatten)]
    output: OutputArgs,
}

#[derive(clap::Subcommand)]
enum RemoteCommand {
    /// List the remotes in ~/.config/otelo/remote.json
    List,
    /// Name the address of an otelo daemon on another machine, so --daemon can query it by name
    Add {
        /// The name, such as mudro
        #[arg(value_parser = parse_remote_name)]
        name: String,
        #[arg(help = "The address of its API, such as https://otelo.mudro.cz")]
        address: DaemonAddress,
    },
    /// Remove a remote, and the password of its address unless another remote has it
    Remove {
        /// The name of the remote
        name: String,
    },
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DaemonAddress(String);

impl FromStr for DaemonAddress {
    type Err = String;

    fn from_str(address: &str) -> Result<Self, Self::Err> {
        if address.starts_with("http://") || address.starts_with("https://") {
            Ok(Self(address.trim_end_matches('/').to_owned()))
        } else {
            Err(format!("{address} is no http:// or https:// address"))
        }
    }
}

impl TryFrom<String> for DaemonAddress {
    type Error = String;

    fn try_from(address: String) -> Result<Self, Self::Error> {
        address.parse()
    }
}

impl From<DaemonAddress> for String {
    fn from(address: DaemonAddress) -> Self {
        address.0
    }
}

impl fmt::Display for DaemonAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Remote {
    pub address: DaemonAddress,
}

type Remotes = BTreeMap<String, Remote>;

fn parse_remote_name(name: &str) -> Result<String, String> {
    if name.is_empty() || name.contains(['/', ':']) {
        Err(format!(
            "{name} is no name of a remote; --daemon reads a value with / or : as an address"
        ))
    } else {
        Ok(name.to_owned())
    }
}

pub fn change_and_print_remotes(args: &RemoteArgs) -> anyhow::Result<()> {
    let remotes_file = find_remotes_file()?;
    let mut remotes = read_remotes(&remotes_file)?;
    match &args.command {
        RemoteCommand::List => print_remotes(&remotes, args.output.choose_output_format())?,
        RemoteCommand::Add { name, address } => {
            if let Some(remote) = remotes.get(name) {
                bail!(
                    "{name} is {} already; `otelo remote remove {name}` removes it",
                    remote.address
                );
            }
            remotes.insert(
                name.clone(),
                Remote {
                    address: address.clone(),
                },
            );
            write_remotes(&remotes_file, &remotes)?;
            eprintln!("`otelo login --daemon {name}` stores its password in the keyring.");
        }
        RemoteCommand::Remove { name } => {
            let removed = remotes
                .remove(name)
                .with_context(|| describe_missing_remote(name))?;
            write_remotes(&remotes_file, &remotes)?;
            let address_has_other_remote = remotes
                .values()
                .any(|remote| remote.address == removed.address);
            if !address_has_other_remote
                && matches!(
                    read_stored_password(&removed.address),
                    StoredPassword::Found(_)
                )
            {
                delete_stored_password(&removed.address)?;
            }
        }
    }
    Ok(())
}

pub fn read_remote(name: &str) -> anyhow::Result<Remote> {
    read_remotes(&find_remotes_file()?)?
        .remove(name)
        .with_context(|| describe_missing_remote(name))
}

fn describe_missing_remote(name: &str) -> String {
    format!("no remote is named {name}; `otelo remote add {name} <ADDRESS>` adds it")
}

fn print_remotes(remotes: &Remotes, output_format: OutputFormat) -> anyhow::Result<()> {
    match output_format {
        OutputFormat::Table => {
            let mut table = Table::new(&["NAME", "ADDRESS"]);
            for (name, remote) in remotes {
                table.add_row(vec![name.clone(), remote.address.to_string()]);
            }
            table.print()?;
        }
        OutputFormat::Json => print_json(remotes)?,
    }
    Ok(())
}

fn find_remotes_file() -> anyhow::Result<PathBuf> {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
        .context("find the config directory: neither XDG_CONFIG_HOME nor HOME is set")?;
    Ok(config_dir.join("otelo").join("remote.json"))
}

fn read_remotes(remotes_file: &Path) -> anyhow::Result<Remotes> {
    match std::fs::read(remotes_file) {
        Ok(json) => serde_json::from_slice(&json)
            .with_context(|| format!("read the remotes in {}", remotes_file.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Remotes::new()),
        Err(error) => Err(error).with_context(|| format!("read {}", remotes_file.display())),
    }
}

fn write_remotes(remotes_file: &Path, remotes: &Remotes) -> anyhow::Result<()> {
    if let Some(config_dir) = remotes_file.parent() {
        std::fs::create_dir_all(config_dir)
            .with_context(|| format!("make {}", config_dir.display()))?;
    }
    let mut json = serde_json::to_string_pretty(remotes)?;
    json.push('\n');
    std::fs::write(remotes_file, json).with_context(|| format!("write {}", remotes_file.display()))
}
