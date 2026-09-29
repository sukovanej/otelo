use std::io::{self, IsTerminal, Write};

use anyhow::{Context, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use otelo_api::ErrorBody;

const MAX_RESPONSE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(clap::Args)]
pub struct Client {
    /// Address of the otelo daemon
    #[arg(
        long,
        env = "OTELO_URL",
        default_value = "http://127.0.0.1:7070",
        global = true
    )]
    daemon: String,

    /// Print JSON, the default when stdout is not a terminal
    #[arg(long, conflicts_with = "table", global = true)]
    json: bool,

    /// Print a table, the default when stdout is a terminal
    #[arg(long, global = true)]
    table: bool,
}

impl Client {
    #[must_use]
    pub fn wants_table(&self) -> bool {
        if self.json {
            false
        } else {
            self.table || io::stdout().is_terminal()
        }
    }

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
            .with_context(|| format!("reach the otelo daemon at {}", self.daemon))?;
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
            .with_context(|| format!("reach the otelo daemon at {}", self.daemon))?;
        read(response)
    }

    pub fn put<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let response = Self::agent()
            .put(self.url(path))
            .send_empty()
            .with_context(|| format!("reach the otelo daemon at {}", self.daemon))?;
        read(response)
    }

    pub fn delete<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let response = Self::agent()
            .delete(self.url(path))
            .call()
            .with_context(|| format!("reach the otelo daemon at {}", self.daemon))?;
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
    let body = response.body_mut().with_config().limit(MAX_RESPONSE_BYTES);
    if status.is_success() {
        return body.read_json().context("read the response of the daemon");
    }
    match body.read_json::<ErrorBody>() {
        Ok(error) => bail!("{}", error.error),
        Err(_) => bail!("the daemon answered {status}"),
    }
}

#[must_use]
pub fn escape_path_segment(segment: &str) -> String {
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

pub fn print_json(value: &impl Serialize) -> anyhow::Result<()> {
    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, value)?;
    writeln!(out)?;
    Ok(())
}

pub fn note_cut(truncated: bool, message: &str) {
    if truncated {
        eprintln!("{message}");
    }
}
