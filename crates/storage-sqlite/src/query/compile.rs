use std::collections::BTreeSet;
use std::fmt;

use otelo_query::{Builtin, Expr, Field, Op, Query, Signal, Value};
use rusqlite::types::Value as Sql;

use otelo_storage::{IndexedAttribute, IndexedSignal, Severity, SpanId, SpanStatus, TraceId};

use super::WhereClause;
use crate::indexes::attribute_json_path;

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

#[derive(Clone, Copy)]
pub struct TableAliases {
    pub record: &'static str,
    pub resource: &'static str,
}

pub fn compile_query(
    query: &Query,
    aliases: TableAliases,
    indexed_attributes: &BTreeSet<IndexedAttribute>,
    param_prefix: &str,
    filter: &mut WhereClause,
) -> Result<Vec<String>> {
    let Some(expr) = &query.expr else {
        return Ok(Vec::new());
    };
    let mut compiler = Compiler {
        signal: query.signal,
        aliases,
        param_prefix,
        filter,
        next_param: 0,
    };
    let sql = compiler.expr(expr)?;
    compiler.filter.push_clause(sql);
    let mut unindexed = Vec::new();
    if let Ok(signal) = IndexedSignal::try_from(query.signal) {
        for field in query.fields() {
            if let Field::Attribute(key) = field {
                let indexed = IndexedAttribute::new(signal, key)
                    .is_ok_and(|attribute| indexed_attributes.contains(&attribute));
                if !indexed && !unindexed.contains(key) {
                    unindexed.push(key.clone());
                }
            }
        }
    }
    Ok(unindexed)
}

struct Compiler<'a> {
    signal: Signal,
    aliases: TableAliases,
    param_prefix: &'a str,
    filter: &'a mut WhereClause,
    next_param: usize,
}

impl Compiler<'_> {
    fn bind_param(&mut self, value: impl Into<Sql>) -> String {
        let name = format!(":{}{}", self.param_prefix, self.next_param);
        self.next_param += 1;
        self.filter.push_param(&name, value);
        name
    }

    fn expr(&mut self, expr: &Expr) -> Result<String> {
        Ok(match expr {
            Expr::And(terms) => self.join(terms, " AND ")?,
            Expr::Or(terms) => self.join(terms, " OR ")?,
            // A comparison of a missing attribute is NULL, and NOT has to keep that record.
            Expr::Not(term) => format!("NOT coalesce({}, FALSE)", self.expr(term)?),
            Expr::Compare { field, op, value } => self.compare(field, *op, value)?,
            Expr::In { field, values } => self.within(field, values)?,
            Expr::Contains { field, text } => self.contains(field, text)?,
            Expr::Has(field) => match field {
                Field::Builtin(builtin) => {
                    return invalid(format!("has() takes an attribute, not {}", builtin.name()));
                }
                _ => format!("json_type({}) IS NOT NULL", self.json_extract_args(field)?),
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

    fn json_extract_args(&self, field: &Field) -> Result<String> {
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
        // The expression of the attribute's index, so SQLite uses the index when there is one.
        Ok(format!("{alias}.{column}, {}", attribute_json_path(key)))
    }

    fn compare(&mut self, field: &Field, op: Op, value: &Value) -> Result<String> {
        if let Field::Builtin(builtin) = field {
            return self.builtin(*builtin, op, value);
        }
        let expr = format!("json_extract({})", self.json_extract_args(field)?);
        Ok(match op {
            Op::Eq => self.equals_any(&expr, &equal_sql_values(value)),
            Op::Ne => format!(
                "({expr} IS NULL OR NOT {})",
                self.equals_any(&expr, &equal_sql_values(value))
            ),
            _ => {
                let param = self.bind_param(value_as_scalar(value));
                format!("{expr} {} {param}", op.as_str())
            }
        })
    }

    fn equals_any(&mut self, expr: &str, values: &[Sql]) -> String {
        let params: Vec<String> = values.iter().map(|v| self.bind_param(v.clone())).collect();
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
        let expr = format!("json_extract({})", self.json_extract_args(field)?);
        let all: Vec<Sql> = values.iter().flat_map(equal_sql_values).collect();
        Ok(self.equals_any(&expr, &all))
    }

    fn contains(&mut self, field: &Field, text: &str) -> Result<String> {
        let column = match field {
            Field::Builtin(Builtin::Body) => {
                let Some(words) = super::logs::quote_fts_words(text) else {
                    return Ok("TRUE".into());
                };
                let param = self.bind_param(words);
                return Ok(format!(
                    "{}.rowid IN (SELECT rowid FROM $day.logs_fts WHERE logs_fts MATCH {param})",
                    self.aliases.record
                ));
            }
            Field::Builtin(builtin) if builtin.text() => self.builtin_column(*builtin),
            Field::Builtin(builtin) => {
                return invalid(format!(
                    "~ takes a text field, and {} is not one",
                    builtin.name()
                ));
            }
            _ => format!("json_extract({})", self.json_extract_args(field)?),
        };
        let param = self.bind_param(text.to_owned());
        Ok(format!("instr({column}, {param}) > 0"))
    }

    fn builtin_column(&self, builtin: Builtin) -> String {
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
        let column = self.builtin_column(builtin);
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
                let param = self.bind_param(value_as_text(value));
                Ok(format!("{column} {} {param}", op.as_str()))
            }
            Builtin::Kind if self.signal == Signal::Metrics => {
                ordered(op)?;
                let param = self.bind_param(value_as_text(value));
                Ok(format!("{column} {} {param}", op.as_str()))
            }
            Builtin::Level => level_clause(&column, op, value),
            Builtin::TraceId | Builtin::SpanId => {
                ordered(op)?;
                let id = match value {
                    Value::String(hex) if builtin == Builtin::TraceId => TraceId::parse_hex(hex)
                        .map(|id| id.0.to_vec())
                        .map_err(|e| InvalidQuery(e.to_string()))?,
                    Value::String(hex) => SpanId::parse_hex(hex)
                        .map(|id| id.0.to_vec())
                        .map_err(|e| InvalidQuery(e.to_string()))?,
                    _ => return invalid(format!("{name} takes hex digits, not {value}")),
                };
                let param = self.bind_param(id);
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
                    (Builtin::Error, true) => {
                        format!("{column} = {}", SpanStatus::Error.number())
                    }
                    (Builtin::Error, false) => {
                        format!("{column} != {}", SpanStatus::Error.number())
                    }
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
                let param = self.bind_param(ns);
                Ok(format!("{column} {} {param}", op.as_str()))
            }
        }
    }
}

fn level_clause(column: &str, op: Op, value: &Value) -> Result<String> {
    let (low, high) = match value {
        Value::Int(n) => {
            let n =
                i32::try_from(*n).map_err(|_| InvalidQuery(format!("{n} is not a severity")))?;
            (n, n)
        }
        Value::String(level) => {
            let low = Severity::parse(level)
                .map_err(|e| InvalidQuery(e.to_string()))?
                .number();
            // A level name covers its four severity numbers.
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

fn value_as_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn value_as_scalar(value: &Value) -> Sql {
    match value {
        Value::String(text) => Sql::Text(text.clone()),
        Value::Int(n) | Value::Duration(n) => Sql::Integer(*n),
        Value::Float(x) => Sql::Real(*x),
        Value::Bool(b) => Sql::Integer(i64::from(*b)),
    }
}

fn equal_sql_values(value: &Value) -> Vec<Sql> {
    // Apps send a number both as a number and as a string.
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
