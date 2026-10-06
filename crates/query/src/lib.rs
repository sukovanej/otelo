mod complete;
mod context;
mod highlight;
mod lexer;
mod parser;

use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub use complete::{
    Catalog, Completion, FieldHelp, FieldOrigin, FieldValues, HelpValue, KeyInfo, MAX_HELP_VALUES,
    NoCatalog, Suggestion, SuggestionKind, ValueInfo, complete_query,
};
pub use highlight::{Highlight, HighlightKind, highlight_tokens};
pub use parser::{ParseError, parse_query, resolve_field};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Signal {
    Logs,
    Spans,
    Metrics,
}

impl Signal {
    pub const ALL: [Self; 3] = [Self::Logs, Self::Spans, Self::Metrics];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Logs => "logs",
            Self::Spans => "spans",
            Self::Metrics => "metrics",
        }
    }

    #[must_use]
    pub const fn builtin_fields(self) -> &'static [BuiltinField] {
        use BuiltinField::{
            Body, Duration, Error, Kind, Level, Name, Root, Service, SpanId, Status, TraceId, Unit,
        };
        match self {
            Self::Logs => &[Service, Level, Body, TraceId, SpanId],
            Self::Spans => &[
                Service, Name, Kind, Status, Error, Duration, Root, TraceId, SpanId,
            ],
            Self::Metrics => &[Name, Service, Kind, Unit],
        }
    }
}

impl std::str::FromStr for Signal {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|signal| signal.name() == text)
            .ok_or_else(|| format!("{text:?} is not a signal: logs, spans, or metrics"))
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BuiltinField {
    Service,
    Level,
    Body,
    TraceId,
    SpanId,
    Name,
    Kind,
    Status,
    Error,
    Duration,
    Root,
    Unit,
}

impl BuiltinField {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::Level => "level",
            Self::Body => "body",
            Self::TraceId => "trace_id",
            Self::SpanId => "span_id",
            Self::Name => "name",
            Self::Kind => "kind",
            Self::Status => "status",
            Self::Error => "error",
            Self::Duration => "duration",
            Self::Root => "root",
            Self::Unit => "unit",
        }
    }

    #[must_use]
    pub fn find_by_name(signal: Signal, name: &str) -> Option<Self> {
        signal
            .builtin_fields()
            .iter()
            .copied()
            .find(|builtin_field| builtin_field.name() == name)
    }

    #[must_use]
    pub const fn fixed_values(self, signal: Signal) -> &'static [&'static str] {
        match (self, signal) {
            (Self::Level, _) => &["trace", "debug", "info", "warn", "error", "fatal"],
            (Self::Kind, Signal::Spans) => {
                &["internal", "server", "client", "producer", "consumer"]
            }
            (Self::Kind, _) => &["gauge", "updown", "counter", "histogram"],
            (Self::Status, _) => &["unset", "ok", "error"],
            (Self::Error | Self::Root, _) => &["true", "false"],
            _ => &[],
        }
    }

    #[must_use]
    pub const fn value_type(self) -> ValueType {
        match self {
            Self::Error | Self::Root => ValueType::Bool,
            Self::Duration => ValueType::Duration,
            _ => ValueType::String,
        }
    }

    #[must_use]
    pub const fn description(self, signal: Signal) -> &'static str {
        match (self, signal) {
            (Self::Service, _) => {
                "The service that sent it, taken from service.name of its resource."
            }
            (Self::Level, _) => {
                "The severity of the log line. Levels compare in order, so level >= warn also finds error and fatal."
            }
            (Self::Body, _) => {
                "The message of the log line. body ~ \"words\" finds the lines that have every word."
            }
            (Self::TraceId, Signal::Logs) => "The trace the log line belongs to, as hex digits.",
            (Self::TraceId, _) => "The trace the span belongs to, as hex digits.",
            (Self::SpanId, Signal::Logs) => "The span the log line was written in, as hex digits.",
            (Self::SpanId, _) => "The id of the span, as hex digits.",
            (Self::Name, Signal::Metrics) => "The name of the metric.",
            (Self::Name, _) => "The name of the span, such as GET /users.",
            (Self::Kind, Signal::Metrics) => {
                "Whether the metric is a gauge, an updown level, a counter, or a histogram."
            }
            (Self::Kind, _) => {
                "The role of the span. It serves a request, makes one, or works inside the service."
            }
            (Self::Status, _) => "How the span ended. Most spans leave it unset.",
            (Self::Error, _) => "Whether the span failed, which means its status is error.",
            (Self::Duration, _) => {
                "How long the span took. It takes a unit, such as duration > 250ms, and a number without one is nanoseconds."
            }
            (Self::Root, _) => "Whether the span starts its trace, which means it has no parent.",
            (Self::Unit, _) => "The unit of the metric, such as ms or By.",
        }
    }

    #[must_use]
    pub const fn is_ordered(self) -> bool {
        matches!(self, Self::Level | Self::Duration)
    }

    #[must_use]
    pub const fn is_text(self) -> bool {
        matches!(self, Self::Body | Self::Name | Self::Service)
    }

    // A log written outside a span has no trace or span.
    #[must_use]
    pub const fn is_on_every_record(self, signal: Signal) -> bool {
        !matches!((self, signal), (Self::TraceId | Self::SpanId, Signal::Logs))
    }
}

/// The type of the values of a field. `mixed` when the values of an
/// attribute have more than one type, and `duration` only for a built-in
/// field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    Null,
    Bool,
    Int,
    Float,
    String,
    Array,
    Object,
    Mixed,
    Duration,
}

impl ValueType {
    pub const ALL: [Self; 9] = [
        Self::Null,
        Self::Bool,
        Self::Int,
        Self::Float,
        Self::String,
        Self::Array,
        Self::Object,
        Self::Mixed,
        Self::Duration,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::Array => "array",
            Self::Object => "object",
            Self::Mixed => "mixed",
            Self::Duration => "duration",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|value_type| value_type.name() == name)
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Builtin(BuiltinField),
    Attribute(String),
    Resource(String),
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin(builtin_field) => f.write_str(builtin_field.name()),
            Self::Attribute(key) if lexer::needs_no_backticks(key) => {
                if key.starts_with("resource.")
                    || key.starts_with("attr.")
                    || lexer::is_keyword(key)
                    || Signal::ALL
                        .iter()
                        .any(|&signal| BuiltinField::find_by_name(signal, key).is_some())
                {
                    write!(f, "attr.{key}")
                } else {
                    f.write_str(key)
                }
            }
            Self::Attribute(key) => write!(f, "`{key}`"),
            Self::Resource(key) => write!(f, "resource.{key}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Operator {
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Duration(i64),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(text) => write!(f, "{}", quote_string(text)),
            Self::Int(integer) => write!(f, "{integer}"),
            Self::Float(float) => write!(f, "{float:?}"),
            Self::Bool(flag) => write!(f, "{flag}"),
            Self::Duration(nanos) => write!(f, "{nanos}ns"),
        }
    }
}

#[must_use]
pub fn quote_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        match character {
            '"' | '\\' => {
                quoted.push('\\');
                quoted.push(character);
            }
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
    Compare {
        field: Field,
        operator: Operator,
        value: Value,
    },
    In {
        field: Field,
        values: Vec<Value>,
    },
    Contains {
        field: Field,
        text: String,
    },
    Has(Field),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    TopLevel,
    Or,
    And,
    Not,
}

impl Expression {
    pub fn visit_fields<'a>(&'a self, visit_field: &mut impl FnMut(&'a Field)) {
        match self {
            Self::And(terms) | Self::Or(terms) => {
                for term in terms {
                    term.visit_fields(visit_field);
                }
            }
            Self::Not(term) => term.visit_fields(visit_field),
            Self::Compare { field, .. }
            | Self::In { field, .. }
            | Self::Contains { field, .. }
            | Self::Has(field) => visit_field(field),
        }
    }

    fn fmt_inside(&self, f: &mut fmt::Formatter<'_>, parent_precedence: Precedence) -> fmt::Result {
        let (terms, separator, precedence) = match self {
            Self::Or(terms) => (terms, " OR ", Precedence::Or),
            Self::And(terms) => (terms, " AND ", Precedence::And),
            Self::Not(term) => {
                f.write_str("NOT ")?;
                return term.fmt_inside(f, Precedence::Not);
            }
            Self::Compare {
                field,
                operator,
                value,
            } => {
                return write!(f, "{field} {} {value}", operator.symbol());
            }
            Self::In { field, values } => {
                let values: Vec<String> = values.iter().map(ToString::to_string).collect();
                return write!(f, "{field} in ({})", values.join(", "));
            }
            Self::Contains { field, text } => return write!(f, "{field} ~ {}", quote_string(text)),
            Self::Has(field) => return write!(f, "has({field})"),
        };
        let needs_parentheses = precedence < parent_precedence;
        if needs_parentheses {
            f.write_str("(")?;
        }
        for (index, term) in terms.iter().enumerate() {
            if index > 0 {
                f.write_str(separator)?;
            }
            term.fmt_inside(f, precedence)?;
        }
        if needs_parentheses {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_inside(f, Precedence::TopLevel)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub signal: Signal,
    pub expression: Option<Expression>,
}

impl Query {
    #[must_use]
    pub const fn all(signal: Signal) -> Self {
        Self {
            signal,
            expression: None,
        }
    }

    #[must_use]
    pub fn fields(&self) -> Vec<&Field> {
        let mut fields = Vec::new();
        if let Some(expression) = &self.expression {
            expression.visit_fields(&mut |field| {
                if !fields.contains(&field) {
                    fields.push(field);
                }
            });
        }
        fields
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expression
            .as_ref()
            .map_or(Ok(()), |expression| expression.fmt(f))
    }
}
