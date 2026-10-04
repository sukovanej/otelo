use std::io::{self, IsTerminal};

use anyhow::{Context, bail};

use super::client::DaemonArgs;
use super::remote::DaemonAddress;

const KEYRING_SERVICE: &str = "otelo";

#[derive(clap::Args)]
pub struct LoginArgs {
    #[command(flatten)]
    daemon: DaemonArgs,
}

#[derive(clap::Args)]
pub struct LogoutArgs {
    #[command(flatten)]
    daemon: DaemonArgs,
}

pub enum StoredPassword {
    Found(String),
    Missing,
    KeyringUnavailable(keyring::Error),
}

pub fn store_daemon_password(args: &LoginArgs) -> anyhow::Result<()> {
    let address = args.daemon.resolve_address()?;
    let entry = open_keyring_entry(&address)?;
    let password = read_password(&format!("Password of {address}: "))?;
    entry
        .set_password(&password)
        .map_err(|error| describe_unavailable_keyring(&error))
}

pub fn delete_daemon_password(args: &LogoutArgs) -> anyhow::Result<()> {
    let address = args.daemon.resolve_address()?;
    match read_stored_password(&address) {
        StoredPassword::Found(_) => delete_stored_password(&address),
        StoredPassword::Missing => bail!("the keyring has no password of {address}"),
        StoredPassword::KeyringUnavailable(error) => Err(describe_unavailable_keyring(&error)),
    }
}

#[must_use]
pub fn read_stored_password(address: &DaemonAddress) -> StoredPassword {
    let password = keyring::Entry::new(KEYRING_SERVICE, &address.to_string())
        .and_then(|entry| entry.get_password());
    match password {
        Ok(password) => StoredPassword::Found(password),
        Err(keyring::Error::NoEntry) => StoredPassword::Missing,
        Err(error) => StoredPassword::KeyringUnavailable(error),
    }
}

pub fn delete_stored_password(address: &DaemonAddress) -> anyhow::Result<()> {
    open_keyring_entry(address)?
        .delete_credential()
        .map_err(|error| describe_unavailable_keyring(&error))
}

fn open_keyring_entry(address: &DaemonAddress) -> anyhow::Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &address.to_string())
        .map_err(|error| describe_unavailable_keyring(&error))
}

fn describe_unavailable_keyring(error: &keyring::Error) -> anyhow::Error {
    anyhow::anyhow!("the keyring is unavailable ({error}); set OTELO_PASSWORD instead")
}

fn read_password(prompt: &str) -> anyhow::Result<String> {
    let password = if io::stdin().is_terminal() {
        rpassword::prompt_password(prompt).context("read the password from the terminal")?
    } else {
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .context("read the password from stdin")?;
        line
    };
    let password = password.trim();
    if password.is_empty() {
        bail!("the password is empty");
    }
    Ok(password.to_owned())
}
