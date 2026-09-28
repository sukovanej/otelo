//! The CLI end of the query API: the daemon address, the HTTP calls, and the
//! choice between a table and JSON.

use std::io::{self, IsTerminal, Write};

use anyhow::{Context, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::api::ErrorBody;

/// The largest response the CLI reads.
const MAX_RESPONSE: u64 = 256 * 1024 * 1024;

#[derive(clap::Args)]
pub struct Client {
    /// Address of the siner daemon
    #[arg(long, env = "SINER_URL", default_value = "http://127.0.0.1:7070")]
    daemon: String,

    /// Print JSON, the default when stdout is not a terminal
    #[arg(long, conflicts_with = "table")]
    json: bool,

    /// Print a table, the default when stdout is a terminal
    #[arg(long)]
    table: bool,
}

impl Client {
    /// Whether to print a table and not JSON.
    pub fn wants_table(&self) -> bool {
        if self.json {
            false
        } else {
            self.table || io::stdout().is_terminal()
        }
    }

    /// GETs `path` with the query parameters that have a value.
    pub fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, Option<String>)],
    ) -> anyhow::Result<T> {
        let pairs = params
            .iter()
            .filter_map(|(name, value)| Some((*name, value.as_deref()?)));
        let response = Self::agent()
            .get(self.url(path))
            .query_pairs(pairs)
            .call()
            .with_context(|| format!("reach the siner daemon at {}", self.daemon))?;
        read(response)
    }

    pub fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> anyhow::Result<T> {
        let response = Self::agent()
            .post(self.url(path))
            .send_json(body)
            .with_context(|| format!("reach the siner daemon at {}", self.daemon))?;
        read(response)
    }

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into()
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.daemon.trim_end_matches('/'))
    }
}

fn read<T: DeserializeOwned>(mut response: ureq::http::Response<ureq::Body>) -> anyhow::Result<T> {
    let status = response.status();
    let body = response.body_mut().with_config().limit(MAX_RESPONSE);
    if status.is_success() {
        return body.read_json().context("read the response of the daemon");
    }
    match body.read_json::<ErrorBody>() {
        Ok(error) => bail!("{}", error.error),
        Err(_) => bail!("the daemon answered {status}"),
    }
}

/// Escapes `segment` for a URL path.
pub fn path_segment(segment: &str) -> String {
    segment
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Prints `value` as JSON on stdout.
pub fn print_json(value: &impl Serialize) -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    writeln!(out)?;
    Ok(())
}

/// Tells on stderr that the result was cut, and how to narrow it.
pub fn note_cut(truncated: bool, message: &str) {
    if truncated {
        eprintln!("{message}");
    }
}
