//! Turns a [`Query`] into the conditions of a `WHERE` clause.
//!
//! A comparison of a record attribute reads
//! `json_extract(<alias>.attributes, '$."key"')`, the expression of its index,
//! so SQLite uses the index when there is one. A number also matches the same
//! number as a string, because apps send both.

use std::collections::BTreeSet;
use std::fmt;

use rusqlite::types::Value as Sql;
use siner_query::{Builtin, Expr, Field, Op, Query, Signal, Value};

use super::{Filter, parse_severity, parse_trace_id};
use crate::indexes::{IndexedKey, json_path};

/// A query the telemetry cannot run, such as a comparison of a field with a
/// value of the wrong kind. The API answers it with 400.
#[derive(Debug)]
pub struct InvalidQuery(pub String);

impl fmt::Display for InvalidQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InvalidQuery {}

type Result<T> = std::result::Result<T, InvalidQuery>;

const fn invalid<T>(message: String) -> Result<T> {
    Err(InvalidQuery(message))
}

/// The table aliases a compiled query reads.
#[derive(Clone, Copy)]
pub struct Aliases {
    /// The log, span, or series.
    pub record: &'static str,
    /// Its resource.
    pub resource: &'static str,
}

/// Adds the conditions of `query` to `filter`, with parameter names that start
/// with `:<prefix>`. Returns the record attributes the query compares that
/// have no index.
pub fn compile(
    query: &Query,
    aliases: Aliases,
    indexes: &BTreeSet<IndexedKey>,
    prefix: &str,
    filter: &mut Filter,
) -> Result<Vec<String>> {
    let Some(expr) = &query.expr else {
        return Ok(Vec::new());
    };
    let mut compiler = Compiler {
        signal: query.signal,
        aliases,
        prefix,
        filter,
        next: 0,
    };
    let sql = compiler.expr(expr)?;
    compiler.filter.push_clause(sql);
    let mut unindexed = Vec::new();
    if matches!(query.signal, Signal::Logs | Signal::Spans) {
        for field in query.fields() {
            if let Field::Attribute(key) = field {
                let wanted = IndexedKey {
                    signal: query.signal,
                    key: key.clone(),
                };
                if !indexes.contains(&wanted) && !unindexed.contains(key) {
                    unindexed.push(key.clone());
                }
            }
        }
    }
    Ok(unindexed)
}

struct Compiler<'a> {
    signal: Signal,
    aliases: Aliases,
    prefix: &'a str,
    filter: &'a mut Filter,
    next: usize,
}

impl Compiler<'_> {
    /// Binds `value` and returns its parameter name.
    fn bind(&mut self, value: impl Into<Sql>) -> String {
        let name = format!(":{}{}", self.prefix, self.next);
        self.next += 1;
        self.filter.param(&name, value);
        name
    }

    fn expr(&mut self, expr: &Expr) -> Result<String> {
        Ok(match expr {
            Expr::And(terms) => self.join(terms, " AND ")?,
            Expr::Or(terms) => self.join(terms, " OR ")?,
            // A comparison of a missing attribute is NULL, and NOT has to keep
            // that record.
            Expr::Not(term) => format!("NOT coalesce({}, FALSE)", self.expr(term)?),
            Expr::Compare { field, op, value } => self.compare(field, *op, value)?,
            Expr::In { field, values } => self.within(field, values)?,
            Expr::Contains { field, text } => self.contains(field, text)?,
            Expr::Has(field) => match field {
                Field::Builtin(builtin) => {
                    return invalid(format!("has() takes an attribute, not {}", builtin.name()));
                }
                _ => format!("json_type({}) IS NOT NULL", self.json_args(field)?),
            },
        })
    }

    fn join(&mut self, terms: &[Expr], join: &str) -> Result<String> {
        let terms = terms
            .iter()
            .map(|term| Ok(format!("({})", self.expr(term)?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(terms.join(join))
    }

    /// The arguments of `json_extract` for an attribute.
    fn json_args(&self, field: &Field) -> Result<String> {
        let (alias, column, key) = match field {
            Field::Attribute(key) => (
                self.aliases.record,
                if self.signal == Signal::Metrics {
                    "labels"
                } else {
                    "attributes"
                },
                key,
            ),
            Field::Resource(key) => (self.aliases.resource, "attributes", key),
            Field::Builtin(_) => unreachable!("a built-in field is a column"),
        };
        if key.contains('"') {
            return invalid(format!(
                "the key {key:?} has a double quote, which a query cannot read"
            ));
        }
        Ok(format!("{alias}.{column}, {}", json_path(key)))
    }

    fn compare(&mut self, field: &Field, op: Op, value: &Value) -> Result<String> {
        if let Field::Builtin(builtin) = field {
            return self.builtin(*builtin, op, value);
        }
        let expr = format!("json_extract({})", self.json_args(field)?);
        Ok(match op {
            Op::Eq => self.in_list(&expr, &variants(value)),
            Op::Ne => format!(
                "({expr} IS NULL OR NOT {})",
                self.in_list(&expr, &variants(value))
            ),
            _ => {
                let param = self.bind(scalar(value));
                format!("{expr} {} {param}", op.as_str())
            }
        })
    }

    /// `expr = value`, or `expr IN (...)` for more values.
    fn in_list(&mut self, expr: &str, values: &[Sql]) -> String {
        let params: Vec<String> = values.iter().map(|v| self.bind(v.clone())).collect();
        if let [param] = params.as_slice() {
            format!("{expr} = {param}")
        } else {
            format!("{expr} IN ({})", params.join(", "))
        }
    }

    fn within(&mut self, field: &Field, values: &[Value]) -> Result<String> {
        if let Field::Builtin(_) = field {
            let terms = values
                .iter()
                .map(|value| Ok(format!("({})", self.compare(field, Op::Eq, value)?)))
                .collect::<Result<Vec<_>>>()?;
            return Ok(terms.join(" OR "));
        }
        let expr = format!("json_extract({})", self.json_args(field)?);
        let all: Vec<Sql> = values.iter().flat_map(variants).collect();
        Ok(self.in_list(&expr, &all))
    }

    fn contains(&mut self, field: &Field, text: &str) -> Result<String> {
        let column = match field {
            Field::Builtin(Builtin::Body) => {
                let Some(words) = super::logs::fts_query(text) else {
                    return Ok("TRUE".into());
                };
                let param = self.bind(words);
                return Ok(format!(
                    "{}.rowid IN (SELECT rowid FROM $day.logs_fts WHERE logs_fts MATCH {param})",
                    self.aliases.record
                ));
            }
            Field::Builtin(builtin) if builtin.text() => self.column(*builtin),
            Field::Builtin(builtin) => {
                return invalid(format!(
                    "~ takes a text field, and {} is not one",
                    builtin.name()
                ));
            }
            _ => format!("json_extract({})", self.json_args(field)?),
        };
        let param = self.bind(text.to_owned());
        Ok(format!("instr({column}, {param}) > 0"))
    }

    /// The column of a built-in field of the signal.
    fn column(&self, builtin: Builtin) -> String {
        let (r, res) = (self.aliases.record, self.aliases.resource);
        match builtin {
            Builtin::Service => format!("{res}.service"),
            Builtin::Level => format!("{r}.severity"),
            Builtin::Body => format!("{r}.body"),
            Builtin::TraceId => format!("{r}.trace_id"),
            Builtin::SpanId => format!("{r}.span_id"),
            Builtin::Source => format!("{r}.source"),
            Builtin::Name => format!("{r}.name"),
            Builtin::Kind => format!("{r}.kind"),
            Builtin::Status | Builtin::Error => format!("{r}.status"),
            Builtin::Duration => format!("{r}.duration_ns"),
            Builtin::Root => format!("{r}.parent_span_id"),
            Builtin::Unit => format!("{r}.unit"),
        }
    }

    fn builtin(&mut self, builtin: Builtin, op: Op, value: &Value) -> Result<String> {
        let column = self.column(builtin);
        let name = builtin.name();
        let ordered = |op: Op| -> Result<()> {
            if matches!(op, Op::Eq | Op::Ne) {
                Ok(())
            } else {
                invalid(format!("{name} takes = or !=, not {}", op.as_str()))
            }
        };
        match builtin {
            Builtin::Service | Builtin::Body | Builtin::Source | Builtin::Name | Builtin::Unit => {
                ordered(op)?;
                let param = self.bind(text(value));
                Ok(format!("{column} {} {param}", op.as_str()))
            }
            Builtin::Kind if self.signal == Signal::Metrics => {
                ordered(op)?;
                let param = self.bind(text(value));
                Ok(format!("{column} {} {param}", op.as_str()))
            }
            Builtin::Level => level_clause(&column, op, value),
            Builtin::TraceId | Builtin::SpanId => {
                ordered(op)?;
                let id = match value {
                    Value::String(hex) if builtin == Builtin::TraceId => parse_trace_id(hex)
                        .map(|id| id.to_vec())
                        .map_err(|e| InvalidQuery(e.to_string()))?,
                    Value::String(hex) => parse_span_id(hex)?,
                    _ => return invalid(format!("{name} takes hex digits, not {value}")),
                };
                let param = self.bind(id);
                Ok(format!("{column} {} {param}", op.as_str()))
            }
            Builtin::Kind | Builtin::Status => {
                ordered(op)?;
                let names = builtin.values(self.signal);
                let number = match value {
                    Value::Int(n) => *n,
                    Value::String(word) => names
                        .iter()
                        .position(|n| n.eq_ignore_ascii_case(word))
                        .map(|i| {
                            // Span kinds count from 1, after unspecified.
                            let i = i64::try_from(i).unwrap_or(0);
                            if builtin == Builtin::Kind { i + 1 } else { i }
                        })
                        .ok_or_else(|| {
                            InvalidQuery(format!(
                                "{name} is one of {}, not {word}",
                                names.join(", ")
                            ))
                        })?,
                    _ => {
                        return invalid(format!(
                            "{name} is one of {}, not {value}",
                            names.join(", ")
                        ));
                    }
                };
                Ok(format!("{column} {} {number}", op.as_str()))
            }
            Builtin::Error | Builtin::Root => {
                ordered(op)?;
                let &Value::Bool(wanted) = value else {
                    return invalid(format!("{name} is true or false, not {value}"));
                };
                let wanted = wanted != (op == Op::Ne);
                Ok(match (builtin, wanted) {
                    (Builtin::Error, true) => format!("{column} = {}", super::STATUS_ERROR),
                    (Builtin::Error, false) => format!("{column} != {}", super::STATUS_ERROR),
                    (_, true) => format!("{column} IS NULL"),
                    (_, false) => format!("{column} IS NOT NULL"),
                })
            }
            Builtin::Duration => {
                let ns = match value {
                    Value::Duration(ns) | Value::Int(ns) => *ns,
                    _ => {
                        return invalid(format!(
                            "duration takes a duration such as 500ms, not {value}"
                        ));
                    }
                };
                let param = self.bind(ns);
                Ok(format!("{column} {} {param}", op.as_str()))
            }
        }
    }
}

/// The condition on the severity column of `level <op> value`. A level name
/// covers its four severity numbers.
fn level_clause(column: &str, op: Op, value: &Value) -> Result<String> {
    let (low, high) = match value {
        Value::Int(n) => {
            let n =
                i32::try_from(*n).map_err(|_| InvalidQuery(format!("{n} is not a severity")))?;
            (n, n)
        }
        Value::String(level) => {
            let low = parse_severity(level).map_err(|e| InvalidQuery(e.to_string()))?;
            (low, low + 3)
        }
        _ => return invalid(format!("level takes a level such as warn, not {value}")),
    };
    Ok(match op {
        Op::Eq => format!("{column} BETWEEN {low} AND {high}"),
        Op::Ne => format!("{column} NOT BETWEEN {low} AND {high}"),
        Op::Lt => format!("{column} < {low}"),
        Op::Le => format!("{column} <= {high}"),
        Op::Gt => format!("{column} > {high}"),
        Op::Ge => format!("{column} >= {low}"),
    })
}

fn parse_span_id(hex: &str) -> Result<Vec<u8>> {
    if hex.len() != 16 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return invalid(format!("{hex:?} is not a span ID of 16 hex digits"));
    }
    Ok((0..8)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap_or(0))
        .collect())
}

/// The value as text, for a text column.
fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The value as one SQL value, for `<` and the like.
fn scalar(value: &Value) -> Sql {
    match value {
        Value::String(text) => Sql::Text(text.clone()),
        Value::Int(n) | Value::Duration(n) => Sql::Integer(*n),
        Value::Float(x) => Sql::Real(*x),
        Value::Bool(b) => Sql::Integer(i64::from(*b)),
    }
}

/// The SQL values an attribute can hold when it equals `value`: a number also
/// as a string, and a string of a number also as that number.
fn variants(value: &Value) -> Vec<Sql> {
    match value {
        Value::String(text) => {
            let mut values = vec![Sql::Text(text.clone())];
            if let Ok(n) = text.parse::<i64>() {
                values.push(Sql::Integer(n));
            } else if let Ok(x) = text.parse::<f64>() {
                values.push(Sql::Real(x));
            }
            values
        }
        Value::Int(n) | Value::Duration(n) => vec![Sql::Integer(*n), Sql::Text(n.to_string())],
        Value::Float(x) => vec![Sql::Real(*x), Sql::Text(x.to_string())],
        Value::Bool(b) => vec![Sql::Integer(i64::from(*b))],
    }
}
