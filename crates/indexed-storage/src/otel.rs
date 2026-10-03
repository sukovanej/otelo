use std::fmt;
use std::ops::RangeInclusive;

use anyhow::{Context, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TraceId(pub [u8; 16]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpanId(pub [u8; 8]);

fn parse_hex_bytes<const N: usize>(text: &str, id_name: &str) -> anyhow::Result<[u8; N]> {
    if text.len() != 2 * N || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("{text:?} is not a {id_name} of {} hex digits", 2 * N);
    }
    let mut bytes = [0; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * index..2 * index + 2], 16)?;
    }
    Ok(bytes)
}

fn write_hex(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    bytes.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
}

// OTLP counts an all-zero trace or span ID as invalid.
fn nonzero_id_bytes<const N: usize>(bytes: &[u8]) -> Option<[u8; N]> {
    let id: [u8; N] = bytes.try_into().ok()?;
    id.iter().any(|&byte| byte != 0).then_some(id)
}

impl TraceId {
    pub fn parse_hex(text: &str) -> anyhow::Result<Self> {
        parse_hex_bytes(text, "trace ID").map(Self)
    }

    #[must_use]
    pub fn from_otlp_bytes(bytes: &[u8]) -> Option<Self> {
        nonzero_id_bytes(bytes).map(Self)
    }
}

impl SpanId {
    pub fn parse_hex(text: &str) -> anyhow::Result<Self> {
        parse_hex_bytes(text, "span ID").map(Self)
    }

    #[must_use]
    pub fn from_otlp_bytes(bytes: &[u8]) -> Option<Self> {
        nonzero_id_bytes(bytes).map(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceContext {
    None,
    Trace(TraceId),
    Span { trace_id: TraceId, span_id: SpanId },
}

impl TraceContext {
    #[must_use]
    pub fn from_otlp_bytes(trace_id: &[u8], span_id: &[u8]) -> Self {
        match (
            TraceId::from_otlp_bytes(trace_id),
            SpanId::from_otlp_bytes(span_id),
        ) {
            (Some(trace_id), Some(span_id)) => Self::Span { trace_id, span_id },
            (Some(trace_id), None) => Self::Trace(trace_id),
            (None, _) => Self::None,
        }
    }

    #[must_use]
    pub const fn trace_id(self) -> Option<TraceId> {
        match self {
            Self::None => None,
            Self::Trace(trace_id) | Self::Span { trace_id, .. } => Some(trace_id),
        }
    }

    #[must_use]
    pub const fn span_id(self) -> Option<SpanId> {
        match self {
            Self::None | Self::Trace(_) => None,
            Self::Span { span_id, .. } => Some(span_id),
        }
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
    const ALL: [Self; 6] = [
        Self::Unspecified,
        Self::Internal,
        Self::Server,
        Self::Client,
        Self::Producer,
        Self::Consumer,
    ];

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

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
    const ALL: [Self; 3] = [Self::Unset, Self::Ok, Self::Error];

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.name() == name)
    }

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
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Ok => "ok",
            Self::Error => "error",
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
    const HIGHEST_NUMBER: u8 = 24;

    #[must_use]
    pub const fn from_number(number: i32) -> Self {
        if number >= 0 && number <= Self::HIGHEST_NUMBER as i32 {
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

    #[must_use]
    pub const fn level_number_range(self) -> RangeInclusive<i32> {
        match self.0 {
            1..=4 => 1..=4,
            5..=8 => 5..=8,
            9..=12 => 9..=12,
            13..=16 => 13..=16,
            17..=20 => 17..=20,
            21..=24 => 21..=24,
            _ => 0..=0,
        }
    }

    pub fn parse_level_or_number(text: &str) -> anyhow::Result<Self> {
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
