use std::io::{self, IsTerminal, Write};
use std::str::FromStr;
use std::sync::OnceLock;

use anyhow::{Context, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use otelo_api::ErrorBody;
use ureq::http::StatusCode;
use ureq::http::header::AUTHORIZATION;

use super::login::{StoredPassword, read_stored_password};
use super::remote::{DaemonAddress, read_remote};

const MAX_RESPONSE_BYTES: u64 = 256 * 1024 * 1024;

const PASSWORD_VARIABLE: &str = "OTELO_PASSWORD";

const LOCAL_DAEMON_URL: &str = "http://127.0.0.1:7070";

#[derive(clap::Args)]
pub struct Client {
    #[command(flatten)]
    daemon_args: DaemonArgs,

    #[command(flatten)]
    output: OutputArgs,

    #[arg(skip)]
    daemon: OnceLock<Daemon>,
}

#[derive(clap::Args)]
pub struct DaemonArgs {
    /// Address of the otelo daemon, or the name of a remote from `otelo remote list`. The CLI
    /// sends the password the keyring keeps for that address, or else `OTELO_PASSWORD`
    #[arg(
        long = "daemon",
        value_name = "DAEMON",
        env = "OTELO_URL",
        default_value = LOCAL_DAEMON_URL,
        global = true
    )]
    daemon_choice: DaemonChoice,
}

#[derive(Clone)]
enum DaemonChoice {
    Address(DaemonAddress),
    Remote { name: String },
}

impl FromStr for DaemonChoice {
    type Err = String;

    fn from_str(choice: &str) -> Result<Self, Self::Err> {
        if choice.contains(['/', ':']) {
            Ok(Self::Address(choice.parse()?))
        } else if choice.is_empty() {
            Err("name a remote or give an http:// or https:// address".into())
        } else {
            Ok(Self::Remote {
                name: choice.to_owned(),
            })
        }
    }
}

#[derive(clap::Args)]
pub struct OutputArgs {
    /// Print JSON, the default when stdout is not a terminal
    #[arg(long, conflicts_with = "table", global = true)]
    json: bool,

    /// Print a table, the default when stdout is a terminal
    #[arg(long, global = true)]
    table: bool,
}

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Json,
    Table,
}

struct Daemon {
    url: String,
    password: Option<String>,
    password_advice: String,
}

impl DaemonArgs {
    pub fn resolve_address(&self) -> anyhow::Result<DaemonAddress> {
        match &self.daemon_choice {
            DaemonChoice::Address(address) => Ok(address.clone()),
            DaemonChoice::Remote { name } => Ok(read_remote(name)?.address),
        }
    }

    fn describe_login_command(&self) -> String {
        match &self.daemon_choice {
            DaemonChoice::Address(address) if address.to_string() == LOCAL_DAEMON_URL => {
                "otelo login".into()
            }
            DaemonChoice::Address(address) => format!("otelo login --daemon {address}"),
            DaemonChoice::Remote { name } => format!("otelo login --daemon {name}"),
        }
    }
}

impl OutputArgs {
    #[must_use]
    pub fn choose_output_format(&self) -> OutputFormat {
        if !self.json && (self.table || io::stdout().is_terminal()) {
            OutputFormat::Table
        } else {
            OutputFormat::Json
        }
    }
}

impl Client {
    #[must_use]
    pub fn choose_output_format(&self) -> OutputFormat {
        self.output.choose_output_format()
    }

    pub fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, Option<String>)],
    ) -> anyhow::Result<T> {
        let daemon = self.resolve_daemon()?;
        let set_params = params
            .iter()
            .filter_map(|(name, value)| Some((*name, value.as_deref()?)));
        let request = build_agent()
            .get(daemon.build_url(path))
            .query_pairs(set_params);
        let response = daemon
            .add_password(request)
            .call()
            .with_context(|| daemon.describe_unreachable())?;
        daemon.read_json_response(response)
    }

    pub fn put<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let daemon = self.resolve_daemon()?;
        let request = build_agent().put(daemon.build_url(path));
        let response = daemon
            .add_password(request)
            .send_empty()
            .with_context(|| daemon.describe_unreachable())?;
        daemon.read_json_response(response)
    }

    pub fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> anyhow::Result<T> {
        let daemon = self.resolve_daemon()?;
        let request = build_agent().post(daemon.build_url(path));
        let response = daemon
            .add_password(request)
            .send_json(body)
            .with_context(|| daemon.describe_unreachable())?;
        daemon.read_json_response(response)
    }

    pub fn put_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> anyhow::Result<T> {
        let daemon = self.resolve_daemon()?;
        let request = build_agent().put(daemon.build_url(path));
        let response = daemon
            .add_password(request)
            .send_json(body)
            .with_context(|| daemon.describe_unreachable())?;
        daemon.read_json_response(response)
    }

    pub fn delete<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let daemon = self.resolve_daemon()?;
        let request = build_agent().delete(daemon.build_url(path));
        let response = daemon
            .add_password(request)
            .call()
            .with_context(|| daemon.describe_unreachable())?;
        daemon.read_json_response(response)
    }

    // A command can send several requests, and each read of the keyring can ask the user.
    fn resolve_daemon(&self) -> anyhow::Result<&Daemon> {
        if let Some(daemon) = self.daemon.get() {
            return Ok(daemon);
        }
        let address = self.daemon_args.resolve_address()?;
        let login_command = self.daemon_args.describe_login_command();
        let (password, password_advice) = match read_stored_password(&address) {
            StoredPassword::Found(password) => (
                Some(password),
                format!("`{login_command}` replaces the password in the keyring"),
            ),
            StoredPassword::Missing => (
                read_password_variable(),
                format!(
                    "`{login_command}` stores the password in the keyring, or set \
                     {PASSWORD_VARIABLE} to it"
                ),
            ),
            StoredPassword::KeyringUnavailable(error) => (
                read_password_variable(),
                format!(
                    "the keyring is unavailable ({error}), so set {PASSWORD_VARIABLE} to the password"
                ),
            ),
        };
        let daemon = Daemon {
            url: address.to_string(),
            password,
            password_advice,
        };
        Ok(self.daemon.get_or_init(|| daemon))
    }
}

impl Daemon {
    fn build_url(&self, path: &str) -> String {
        format!("{}{path}", self.url)
    }

    fn add_password<Body>(
        &self,
        request: ureq::RequestBuilder<Body>,
    ) -> ureq::RequestBuilder<Body> {
        match &self.password {
            Some(password) => request.header(AUTHORIZATION, format!("Bearer {password}")),
            None => request,
        }
    }

    fn describe_unreachable(&self) -> String {
        format!("reach the otelo daemon at {}", self.url)
    }

    fn read_json_response<T: DeserializeOwned>(
        &self,
        mut response: ureq::http::Response<ureq::Body>,
    ) -> anyhow::Result<T> {
        let status = response.status();
        let body = response.body_mut().with_config().limit(MAX_RESPONSE_BYTES);
        if status.is_success() {
            return body.read_json().context("read the response of the daemon");
        }
        let message = body.read_json::<ErrorBody>().map_or_else(
            |_| format!("the daemon answered {status}"),
            |rejection| rejection.error,
        );
        if status == StatusCode::UNAUTHORIZED {
            bail!("{message}; {}", self.password_advice);
        }
        bail!("{message}")
    }
}

fn read_password_variable() -> Option<String> {
    std::env::var(PASSWORD_VARIABLE)
        .ok()
        .map(|password| password.trim().to_owned())
}

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

#[must_use]
pub fn escape_path_segment(segment: &str) -> String {
    segment
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

pub fn print_json(value: &impl Serialize) -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    writeln!(out)?;
    Ok(())
}

pub fn note_truncation(truncated: bool, message: &str) {
    if truncated {
        eprintln!("{message}");
    }
}
