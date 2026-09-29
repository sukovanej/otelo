use std::fmt;

use anyhow::{bail, ensure};
use siner_query::Signal;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IndexedSignal {
    Logs,
    Spans,
}

impl IndexedSignal {
    #[must_use]
    pub const fn signal(self) -> Signal {
        match self {
            Self::Logs => Signal::Logs,
            Self::Spans => Signal::Spans,
        }
    }
}

impl TryFrom<Signal> for IndexedSignal {
    type Error = anyhow::Error;

    fn try_from(signal: Signal) -> anyhow::Result<Self> {
        match signal {
            Signal::Logs => Ok(Self::Logs),
            Signal::Spans => Ok(Self::Spans),
            Signal::Metrics => bail!("only the attributes of logs and spans have indexes"),
        }
    }
}

impl fmt::Display for IndexedSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.signal().fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedAttribute {
    signal: IndexedSignal,
    key: String,
}

impl IndexedAttribute {
    pub fn new(signal: IndexedSignal, key: &str) -> anyhow::Result<Self> {
        ensure!(!key.is_empty(), "the key is empty");
        ensure!(
            !key.contains('"'),
            "a key with a double quote cannot be indexed"
        );
        Ok(Self {
            signal,
            key: key.to_owned(),
        })
    }

    #[must_use]
    pub const fn signal(&self) -> IndexedSignal {
        self.signal
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
}
