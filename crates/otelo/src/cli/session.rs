use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use anyhow::{Context, bail};

pub struct SessionFile {
    path: PathBuf,
}

impl SessionFile {
    pub fn locate() -> anyhow::Result<Self> {
        let config_directory = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(directory) if !directory.is_empty() => PathBuf::from(directory),
            _ => match std::env::var_os("HOME") {
                Some(home) => PathBuf::from(home).join(".config"),
                None => bail!("neither XDG_CONFIG_HOME nor HOME names the config directory"),
            },
        };
        Ok(Self {
            path: config_directory.join("otelo").join("sessions.json"),
        })
    }

    pub fn read_token(&self, daemon_url: &str) -> anyhow::Result<Option<String>> {
        Ok(self.read_tokens_by_daemon_url()?.remove(daemon_url))
    }

    pub fn save_token(&self, daemon_url: &str, token: &str) -> anyhow::Result<()> {
        let mut tokens = self.read_tokens_by_daemon_url()?;
        tokens.insert(daemon_url.to_owned(), token.to_owned());
        self.write_tokens_by_daemon_url(&tokens)
    }

    pub fn remove_token(&self, daemon_url: &str) -> anyhow::Result<Option<String>> {
        let mut tokens = self.read_tokens_by_daemon_url()?;
        let removed = tokens.remove(daemon_url);
        if removed.is_some() {
            self.write_tokens_by_daemon_url(&tokens)?;
        }
        Ok(removed)
    }

    fn read_tokens_by_daemon_url(&self) -> anyhow::Result<BTreeMap<String, String>> {
        match fs::read(&self.path) {
            Ok(json) => serde_json::from_slice(&json)
                .with_context(|| format!("read the sessions in {}", self.path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(error) => Err(error).with_context(|| format!("read {}", self.path.display())),
        }
    }

    // The tokens are secrets, so only their owner reads the file.
    fn write_tokens_by_daemon_url(&self, tokens: &BTreeMap<String, String>) -> anyhow::Result<()> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory)
                .with_context(|| format!("make {}", directory.display()))?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&self.path)
            .with_context(|| format!("open {}", self.path.display()))?;
        serde_json::to_writer_pretty(&mut file, tokens)?;
        writeln!(file)?;
        Ok(())
    }
}
