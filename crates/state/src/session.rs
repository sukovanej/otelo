use std::fmt;
use std::str::FromStr;

use anyhow::{Context, ensure};
use data_encoding::BASE64URL_NOPAD;
use sha2::{Digest, Sha256};

const SESSION_TOKEN_BYTES: usize = 32;

#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken([u8; SESSION_TOKEN_BYTES]);

impl SessionToken {
    pub(crate) fn generate() -> anyhow::Result<Self> {
        let mut bytes = [0; SESSION_TOKEN_BYTES];
        getrandom::fill(&mut bytes).context("generate a session token")?;
        Ok(Self(bytes))
    }

    pub(crate) fn hash(&self) -> [u8; 32] {
        Sha256::digest(self.0).into()
    }
}

impl fmt::Display for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&BASE64URL_NOPAD.encode(&self.0))
    }
}

impl FromStr for SessionToken {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> anyhow::Result<Self> {
        let bytes = BASE64URL_NOPAD
            .decode(text.as_bytes())
            .context("a session token is base64url")?;
        ensure!(
            bytes.len() == SESSION_TOKEN_BYTES,
            "a session token has {SESSION_TOKEN_BYTES} bytes"
        );
        let mut token = [0; SESSION_TOKEN_BYTES];
        token.copy_from_slice(&bytes);
        Ok(Self(token))
    }
}
