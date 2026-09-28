//! The query language that filters logs, spans, and metric series.
//!
//! ```text
//! http.route = "/matches" OR (user.id = 7 AND http.response.status_code = 200)
//! level >= warn body ~ "payment failed"
//! duration > 500ms AND NOT resource.host.name = "droplet" AND has(exception.message)
//! ```
//!
//! A name is a built-in field of the signal (see [`Builtin`]), `resource.`
//! and a resource attribute, `attr.` and a record attribute, or else a record
//! attribute. A key with other characters goes in backticks. Terms next to
//! each other join with `AND`.
//!
//! [`parse`] makes a [`Query`], and [`complete`] suggests what can come at a
//! cursor, with the attributes and values of a [`Catalog`].

mod complete;
mod lexer;
mod parser;

use std::fmt;

pub use complete::{Catalog, KeyInfo, NoCatalog, Suggestion, SuggestionKind, ValueInfo, complete};
pub use parser::{ParseError, parse};

/// The kind of record a query filters.
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

    /// The built-in fields of the signal.
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

/// A field that every record of a signal has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    /// The `service.name` of the resource.
    Service,
    /// The severity of a log: `trace`, `debug`, `info`, `warn`, `error`,
    /// `fatal`, or a number.
    Level,
    /// The body of a log. `~` finds words in it.
    Body,
    TraceId,
    SpanId,
    /// Where a log came from: `otlp`, or a service log source.
    Source,
    /// The name of a span or of a metric.
    Name,
    /// The kind of a span or of a metric.
    Kind,
    /// The status of a span: `unset`, `ok`, or `error`.
    Status,
    /// Whether a span failed.
    Error,
    /// How long a span took, such as `500ms`.
    Duration,
    /// Whether a span has no parent.
    Root,
    /// The unit of a metric.
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

    /// The built-in field of `signal` called `name`.
    #[must_use]
    pub fn find(signal: Signal, name: &str) -> Option<Self> {
        signal
            .builtins()
            .iter()
            .copied()
            .find(|builtin| builtin.name() == name)
    }

    /// The values the field can have, when there is a fixed set.
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

    /// Whether `<`, `<=`, `>`, and `>=` make sense on the field.
    #[must_use]
    pub const fn ordered(self) -> bool {
        matches!(self, Self::Level | Self::Duration)
    }

    /// Whether `~` makes sense on the field.
    #[must_use]
    pub const fn text(self) -> bool {
        matches!(self, Self::Body | Self::Name | Self::Service | Self::Source)
    }
}

/// What a comparison reads.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Builtin(Builtin),
    /// An attribute of the log, the span, or the metric series.
    Attribute(String),
    /// An attribute of the resource that sent the record.
    Resource(String),
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin(builtin) => f.write_str(builtin.name()),
            Self::Attribute(key) if lexer::is_plain_key(key) => {
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
    /// A quoted string, or a bare word such as `warn` or `/matches`.
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    /// A duration such as `500ms`, in nanoseconds.
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

/// `text` in double quotes, with `"` and `\` escaped.
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
    Compare {
        field: Field,
        op: Op,
        value: Value,
    },
    In {
        field: Field,
        values: Vec<Value>,
    },
    /// `~`: the field contains the text. On a log body, it finds the words.
    Contains {
        field: Field,
        text: String,
    },
    /// `has(field)`: the record has the attribute.
    Has(Field),
}

impl Expr {
    /// Calls `each` on every field the expression reads.
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

/// A parsed query. An empty query keeps every record.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub signal: Signal,
    pub expr: Option<Expr>,
}

impl Query {
    /// The query that keeps every record of `signal`.
    #[must_use]
    pub const fn all(signal: Signal) -> Self {
        Self { signal, expr: None }
    }

    /// Every field the query reads, each once, in the order they appear.
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
