use std::io::{self, IsTerminal, Write};

use anyhow::{Context, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use otelo_api::ErrorBody;
use ureq::http::StatusCode;
use ureq::http::header::AUTHORIZATION;

use crate::cli::session::SessionFile;

const MAX_RESPONSE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(clap::Args)]
pub struct DaemonArgs {
    /// Address of the otelo daemon
    #[arg(
        long = "daemon",
        value_name = "DAEMON",
        env = "OTELO_URL",
        default_value = "http://127.0.0.1:7070",
        global = true
    )]
    daemon_url: String,
}

impl DaemonArgs {
    #[must_use]
    pub fn url(&self) -> &str {
        self.daemon_url.trim_end_matches('/')
    }

    #[must_use]
    pub fn build_url(&self, path: &str) -> String {
        format!("{}{path}", self.url())
    }

    #[must_use]
    pub fn reach_error_context(&self) -> String {
        format!("reach the otelo daemon at {}", self.url())
    }
}

#[derive(clap::Args)]
pub struct Client {
    #[command(flatten)]
    daemon: DaemonArgs,

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

impl Client {
    #[must_use]
    pub fn choose_output_format(&self) -> OutputFormat {
        if !self.json && (self.table || io::stdout().is_terminal()) {
            OutputFormat::Table
        } else {
            OutputFormat::Json
        }
    }

    pub fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, Option<String>)],
    ) -> anyhow::Result<T> {
        let set_params = params
            .iter()
            .filter_map(|(name, value)| Some((*name, value.as_deref()?)));
        let request = build_agent()
            .get(self.daemon.build_url(path))
            .query_pairs(set_params);
        let response = self
            .add_session(request)?
            .call()
            .with_context(|| self.daemon.reach_error_context())?;
        read_json_response(response)
    }

    pub fn put<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let request = build_agent().put(self.daemon.build_url(path));
        let response = self
            .add_session(request)?
            .send_empty()
            .with_context(|| self.daemon.reach_error_context())?;
        read_json_response(response)
    }

    pub fn delete<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let request = build_agent().delete(self.daemon.build_url(path));
        let response = self
            .add_session(request)?
            .call()
            .with_context(|| self.daemon.reach_error_context())?;
        read_json_response(response)
    }

    fn add_session<Body>(
        &self,
        request: ureq::RequestBuilder<Body>,
    ) -> anyhow::Result<ureq::RequestBuilder<Body>> {
        let token = SessionFile::locate()?.read_token(self.daemon.url())?;
        Ok(match token {
            Some(token) => request.header(AUTHORIZATION, format!("Bearer {token}")),
            None => request,
        })
    }
}

#[must_use]
pub fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

pub fn read_json_response<T: DeserializeOwned>(
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
        bail!("{message}; `otelo login` starts a session");
    }
    bail!("{message}")
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
