use std::fmt;

use anyhow::{Context, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TraceId(pub [u8; 16]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpanId(pub [u8; 8]);

fn parse_hex_bytes<const N: usize>(text: &str, what: &str) -> anyhow::Result<[u8; N]> {
    if text.len() != 2 * N || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("{text:?} is not a {what} of {} hex digits", 2 * N);
    }
    let mut bytes = [0; N];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)?;
    }
    Ok(bytes)
}

fn write_hex(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))
}

impl TraceId {
    pub fn parse_hex(text: &str) -> anyhow::Result<Self> {
        parse_hex_bytes(text, "trace ID").map(Self)
    }
}

impl SpanId {
    pub fn parse_hex(text: &str) -> anyhow::Result<Self> {
        parse_hex_bytes(text, "span ID").map(Self)
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(&self.0, f)
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(&self.0, f)
    }
}

impl Serialize for TraceId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TraceId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse_hex(&text).map_err(serde::de::Error::custom)
    }
}

impl Serialize for SpanId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for SpanId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse_hex(&text).map_err(serde::de::Error::custom)
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(from = "i32", into = "i32")]
pub enum SpanKind {
    #[default]
    Unspecified,
    Internal,
    Server,
    Client,
    Producer,
    Consumer,
}

impl SpanKind {
    #[must_use]
    pub const fn from_number(number: i32) -> Self {
        match number {
            1 => Self::Internal,
            2 => Self::Server,
            3 => Self::Client,
            4 => Self::Producer,
            5 => Self::Consumer,
            _ => Self::Unspecified,
        }
    }

    #[must_use]
    pub const fn number(self) -> i32 {
        match self {
            Self::Unspecified => 0,
            Self::Internal => 1,
            Self::Server => 2,
            Self::Client => 3,
            Self::Producer => 4,
            Self::Consumer => 5,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::Internal => "internal",
            Self::Server => "server",
            Self::Client => "client",
            Self::Producer => "producer",
            Self::Consumer => "consumer",
        }
    }
}

impl From<i32> for SpanKind {
    fn from(number: i32) -> Self {
        Self::from_number(number)
    }
}

impl From<SpanKind> for i32 {
    fn from(kind: SpanKind) -> Self {
        kind.number()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
pub enum SpanStatus {
    #[default]
    Unset,
    Ok,
    Error,
}

impl SpanStatus {
    #[must_use]
    pub const fn from_number(number: i32) -> Self {
        match number {
            1 => Self::Ok,
            2 => Self::Error,
            _ => Self::Unset,
        }
    }

    #[must_use]
    pub const fn number(self) -> i32 {
        match self {
            Self::Unset => 0,
            Self::Ok => 1,
            Self::Error => 2,
        }
    }

    #[must_use]
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }
}

impl From<i32> for SpanStatus {
    fn from(number: i32) -> Self {
        Self::from_number(number)
    }
}

impl From<SpanStatus> for i32 {
    fn from(status: SpanStatus) -> Self {
        status.number()
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(from = "i32", into = "i32")]
pub struct Severity(u8);

impl Severity {
    pub const UNSPECIFIED: Self = Self(0);
    pub const TRACE: Self = Self(1);
    pub const DEBUG: Self = Self(5);
    pub const INFO: Self = Self(9);
    pub const WARN: Self = Self(13);
    pub const ERROR: Self = Self(17);
    pub const FATAL: Self = Self(21);
    const HIGHEST: u8 = 24;

    #[must_use]
    pub const fn from_number(number: i32) -> Self {
        if number >= 0 && number <= Self::HIGHEST as i32 {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked 0..=24"
            )]
            Self(number as u8)
        } else {
            Self::UNSPECIFIED
        }
    }

    #[must_use]
    pub const fn number(self) -> i32 {
        self.0 as i32
    }

    #[must_use]
    pub const fn level(self) -> &'static str {
        match self.0 {
            1..=4 => "TRACE",
            5..=8 => "DEBUG",
            9..=12 => "INFO",
            13..=16 => "WARN",
            17..=20 => "ERROR",
            21..=24 => "FATAL",
            _ => "UNSPECIFIED",
        }
    }

    pub fn parse(text: &str) -> anyhow::Result<Self> {
        Ok(match text.to_ascii_lowercase().as_str() {
            "trace" => Self::TRACE,
            "debug" => Self::DEBUG,
            "info" => Self::INFO,
            "warn" | "warning" => Self::WARN,
            "error" => Self::ERROR,
            "fatal" => Self::FATAL,
            number => {
                let number: i32 = number.parse().with_context(|| {
                    format!(
                        "{text:?} is not a severity: trace, debug, info, warn, error, fatal, or a number"
                    )
                })?;
                let severity = Self::from_number(number);
                if severity.number() != number {
                    bail!("{number} is not a severity number, which runs from 0 to 24");
                }
                severity
            }
        })
    }
}

impl From<i32> for Severity {
    fn from(number: i32) -> Self {
        Self::from_number(number)
    }
}

impl From<Severity> for i32 {
    fn from(severity: Severity) -> Self {
        severity.number()
    }
}
