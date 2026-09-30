mod complete;
mod highlight;
mod lexer;
mod parser;

use std::fmt;

pub use complete::{
    Catalog, Completion, FieldHelp, FieldOrigin, FieldValues, HelpValue, KeyInfo, MAX_HELP_VALUES,
    NoCatalog, Suggestion, SuggestionKind, ValueInfo, complete,
};
pub use highlight::{Highlight, HighlightKind, highlight_tokens};
pub use parser::{ParseError, parse};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Signal {
    Logs,
    Spans,
    Metrics,
}

impl Signal {
    pub const ALL: [Self; 3] = [Self::Logs, Self::Spans, Self::Metrics];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Logs => "logs",
            Self::Spans => "spans",
            Self::Metrics => "metrics",
        }
    }

    #[must_use]
    pub const fn builtins(self) -> &'static [Builtin] {
        use Builtin::{
            Body, Duration, Error, Kind, Level, Name, Root, Service, Source, SpanId, Status,
            TraceId, Unit,
        };
        match self {
            Self::Logs => &[Service, Level, Body, TraceId, SpanId, Source],
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
            .find(|signal| signal.as_str() == text)
            .ok_or_else(|| format!("{text:?} is not a signal: logs, spans, or metrics"))
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    Service,
    Level,
    Body,
    TraceId,
    SpanId,
    Source,
    Name,
    Kind,
    Status,
    Error,
    Duration,
    Root,
    Unit,
}

impl Builtin {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Service => "service",
            Self::Level => "level",
            Self::Body => "body",
            Self::TraceId => "trace_id",
            Self::SpanId => "span_id",
            Self::Source => "source",
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
    pub fn find(signal: Signal, name: &str) -> Option<Self> {
        signal
            .builtins()
            .iter()
            .copied()
            .find(|builtin| builtin.name() == name)
    }

    #[must_use]
    pub const fn values(self, signal: Signal) -> &'static [&'static str] {
        match (self, signal) {
            (Self::Level, _) => &["trace", "debug", "info", "warn", "error", "fatal"],
            (Self::Kind, Signal::Spans) => {
                &["internal", "server", "client", "producer", "consumer"]
            }
            (Self::Kind, _) => &["gauge", "sum", "histogram"],
            (Self::Status, _) => &["unset", "ok", "error"],
            (Self::Error | Self::Root, _) => &["true", "false"],
            _ => &[],
        }
    }

    #[must_use]
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Error | Self::Root => "bool",
            Self::Duration => "duration",
            _ => "string",
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
            (Self::Source, _) => "How the log line arrived, such as otlp.",
            (Self::Name, Signal::Metrics) => "The name of the metric.",
            (Self::Name, _) => "The name of the span, such as GET /users.",
            (Self::Kind, Signal::Metrics) => {
                "Whether the metric is a gauge, a sum, or a histogram."
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
    pub const fn ordered(self) -> bool {
        matches!(self, Self::Level | Self::Duration)
    }

    #[must_use]
    pub const fn text(self) -> bool {
        matches!(self, Self::Body | Self::Name | Self::Service | Self::Source)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Builtin(Builtin),
    Attribute(String),
    Resource(String),
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin(builtin) => f.write_str(builtin.name()),
            Self::Attribute(key) if lexer::needs_no_backticks(key) => {
                if key.starts_with("resource.")
                    || key.starts_with("attr.")
                    || lexer::is_keyword(key)
                    || Signal::ALL.iter().any(|&s| Builtin::find(s, key).is_some())
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
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Op {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
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
            Self::String(text) => write!(f, "{}", quote(text)),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(x) => write!(f, "{x:?}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Duration(ns) => write!(f, "{ns}ns"),
        }
    }
}

#[must_use]
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    And(Vec<Self>),
    Or(Vec<Self>),
    Not(Box<Self>),
    Compare { field: Field, op: Op, value: Value },
    In { field: Field, values: Vec<Value> },
    Contains { field: Field, text: String },
    Has(Field),
}

impl Expr {
    pub fn visit_fields<'a>(&'a self, each: &mut impl FnMut(&'a Field)) {
        match self {
            Self::And(terms) | Self::Or(terms) => {
                for term in terms {
                    term.visit_fields(each);
                }
            }
            Self::Not(term) => term.visit_fields(each),
            Self::Compare { field, .. }
            | Self::In { field, .. }
            | Self::Contains { field, .. }
            | Self::Has(field) => each(field),
        }
    }

    fn fmt_in(&self, f: &mut fmt::Formatter<'_>, parent: u8) -> fmt::Result {
        let (terms, join, level) = match self {
            Self::Or(terms) => (terms, " OR ", 1),
            Self::And(terms) => (terms, " AND ", 2),
            Self::Not(term) => {
                f.write_str("NOT ")?;
                return term.fmt_in(f, 3);
            }
            Self::Compare { field, op, value } => {
                return write!(f, "{field} {} {value}", op.as_str());
            }
            Self::In { field, values } => {
                let values: Vec<String> = values.iter().map(ToString::to_string).collect();
                return write!(f, "{field} in ({})", values.join(", "));
            }
            Self::Contains { field, text } => return write!(f, "{field} ~ {}", quote(text)),
            Self::Has(field) => return write!(f, "has({field})"),
        };
        let grouped = level < parent;
        if grouped {
            f.write_str("(")?;
        }
        for (i, term) in terms.iter().enumerate() {
            if i > 0 {
                f.write_str(join)?;
            }
            term.fmt_in(f, level)?;
        }
        if grouped {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_in(f, 0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub signal: Signal,
    pub expr: Option<Expr>,
}

impl Query {
    #[must_use]
    pub const fn all(signal: Signal) -> Self {
        Self { signal, expr: None }
    }

    #[must_use]
    pub fn fields(&self) -> Vec<&Field> {
        let mut fields = Vec::new();
        if let Some(expr) = &self.expr {
            expr.visit_fields(&mut |field| {
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
        self.expr.as_ref().map_or(Ok(()), |expr| expr.fmt(f))
    }
}
