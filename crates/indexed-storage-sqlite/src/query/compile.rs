use std::collections::BTreeSet;
use std::fmt;

use otelo_query::{BuiltinField, Expression, Field, Operator, Query, Signal, Value};
use rusqlite::types::Value as SqliteValue;

use otelo_indexed_storage::{
    IndexedAttribute, IndexedSignal, Severity, SpanId, SpanKind, SpanStatus, TraceId,
};

use super::WhereClause;
use crate::indexes::attribute_json_path;

#[derive(Debug)]
pub struct InvalidQuery(pub String);

impl fmt::Display for InvalidQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for InvalidQuery {}

type Result<T> = std::result::Result<T, InvalidQuery>;

const fn invalid_query<T>(message: String) -> Result<T> {
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
    where_clause: &mut WhereClause,
) -> Result<Vec<String>> {
    let Some(expression) = &query.expression else {
        return Ok(Vec::new());
    };
    let mut compiler = Compiler {
        signal: query.signal,
        aliases,
        param_prefix,
        where_clause,
        next_param: 0,
    };
    let sql = compiler.compile_expression(expression)?;
    compiler.where_clause.push_condition(sql);
    let mut unindexed = Vec::new();
    if let Ok(signal) = IndexedSignal::try_from(query.signal) {
        for field in query.fields() {
            if let Field::Attribute(key) = field {
                let is_indexed = IndexedAttribute::new(signal, key)
                    .is_ok_and(|attribute| indexed_attributes.contains(&attribute));
                if !is_indexed && !unindexed.contains(key) {
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
    where_clause: &'a mut WhereClause,
    next_param: usize,
}

impl Compiler<'_> {
    fn bind_param(&mut self, value: impl Into<SqliteValue>) -> String {
        let name = format!(":{}{}", self.param_prefix, self.next_param);
        self.next_param += 1;
        self.where_clause.push_param(&name, value);
        name
    }

    fn compile_expression(&mut self, expression: &Expression) -> Result<String> {
        Ok(match expression {
            Expression::And(terms) => self.compile_joined_terms(terms, " AND ")?,
            Expression::Or(terms) => self.compile_joined_terms(terms, " OR ")?,
            // A comparison of a missing attribute is NULL, and NOT has to keep that record.
            Expression::Not(term) => {
                format!("NOT coalesce({}, FALSE)", self.compile_expression(term)?)
            }
            Expression::Compare {
                field,
                operator,
                value,
            } => self.compile_comparison(field, *operator, value)?,
            Expression::In { field, values } => self.compile_in_list(field, values)?,
            Expression::Contains { field, text } => self.compile_contains(field, text)?,
            Expression::Has(field) => match field {
                Field::Builtin(builtin_field) => {
                    return invalid_query(format!(
                        "has() takes an attribute, not {}",
                        builtin_field.name()
                    ));
                }
                _ => format!("json_type({}) IS NOT NULL", self.json_extract_args(field)?),
            },
        })
    }

    fn compile_joined_terms(&mut self, terms: &[Expression], separator: &str) -> Result<String> {
        let terms = terms
            .iter()
            .map(|term| Ok(format!("({})", self.compile_expression(term)?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(terms.join(separator))
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
            return invalid_query(format!(
                "the key {key:?} has a double quote, which a query cannot read"
            ));
        }
        // The expression of the attribute's index, so SQLite uses the index when there is one.
        Ok(format!("{alias}.{column}, {}", attribute_json_path(key)))
    }

    fn compile_comparison(
        &mut self,
        field: &Field,
        operator: Operator,
        value: &Value,
    ) -> Result<String> {
        if let Field::Builtin(builtin_field) = field {
            return self.compile_builtin_comparison(*builtin_field, operator, value);
        }
        let extracted_sql = format!("json_extract({})", self.json_extract_args(field)?);
        Ok(match operator {
            Operator::Eq => self.compile_equals_any(&extracted_sql, &equal_sql_values(value)),
            Operator::Ne => format!(
                "({extracted_sql} IS NULL OR NOT {})",
                self.compile_equals_any(&extracted_sql, &equal_sql_values(value))
            ),
            _ => {
                let param = self.bind_param(value_as_scalar(value));
                format!("{extracted_sql} {} {param}", operator.symbol())
            }
        })
    }

    fn compile_equals_any(&mut self, extracted_sql: &str, values: &[SqliteValue]) -> String {
        let params: Vec<String> = values
            .iter()
            .map(|value| self.bind_param(value.clone()))
            .collect();
        if let [param] = params.as_slice() {
            format!("{extracted_sql} = {param}")
        } else {
            format!("{extracted_sql} IN ({})", params.join(", "))
        }
    }

    fn compile_in_list(&mut self, field: &Field, values: &[Value]) -> Result<String> {
        if let Field::Builtin(_) = field {
            let terms = values
                .iter()
                .map(|value| {
                    Ok(format!(
                        "({})",
                        self.compile_comparison(field, Operator::Eq, value)?
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            return Ok(terms.join(" OR "));
        }
        let extracted_sql = format!("json_extract({})", self.json_extract_args(field)?);
        let sql_values: Vec<SqliteValue> = values.iter().flat_map(equal_sql_values).collect();
        Ok(self.compile_equals_any(&extracted_sql, &sql_values))
    }

    fn compile_contains(&mut self, field: &Field, text: &str) -> Result<String> {
        let column = match field {
            Field::Builtin(BuiltinField::Body) => {
                let Some(words) = super::logs::quote_fts_words(text) else {
                    return Ok("TRUE".into());
                };
                let param = self.bind_param(words);
                return Ok(format!(
                    "{}.rowid IN (SELECT rowid FROM $day.logs_fts WHERE logs_fts MATCH {param})",
                    self.aliases.record
                ));
            }
            Field::Builtin(builtin_field) if builtin_field.is_text() => {
                self.builtin_column(*builtin_field)
            }
            Field::Builtin(builtin_field) => {
                return invalid_query(format!(
                    "~ takes a text field, and {} is not one",
                    builtin_field.name()
                ));
            }
            _ => format!("json_extract({})", self.json_extract_args(field)?),
        };
        let param = self.bind_param(text.to_owned());
        Ok(format!("instr({column}, {param}) > 0"))
    }

    fn builtin_column(&self, builtin_field: BuiltinField) -> String {
        let (record, resource) = (self.aliases.record, self.aliases.resource);
        match builtin_field {
            BuiltinField::Service => format!("{resource}.service"),
            BuiltinField::Level => format!("{record}.severity"),
            BuiltinField::Body => format!("{record}.body"),
            BuiltinField::TraceId => format!("{record}.trace_id"),
            BuiltinField::SpanId => format!("{record}.span_id"),
            BuiltinField::Name => format!("{record}.name"),
            BuiltinField::Kind => format!("{record}.kind"),
            BuiltinField::Status | BuiltinField::Error => format!("{record}.status"),
            BuiltinField::Duration => format!("{record}.duration_ns"),
            BuiltinField::Root => format!("{record}.parent_span_id"),
            BuiltinField::Unit => format!("{record}.unit"),
        }
    }

    fn compile_builtin_comparison(
        &mut self,
        builtin_field: BuiltinField,
        operator: Operator,
        value: &Value,
    ) -> Result<String> {
        let column = self.builtin_column(builtin_field);
        let name = builtin_field.name();
        let ensure_equality_operator = |operator: Operator| -> Result<()> {
            if matches!(operator, Operator::Eq | Operator::Ne) {
                Ok(())
            } else {
                invalid_query(format!("{name} takes = or !=, not {}", operator.symbol()))
            }
        };
        match builtin_field {
            BuiltinField::Service
            | BuiltinField::Body
            | BuiltinField::Name
            | BuiltinField::Unit => {
                ensure_equality_operator(operator)?;
                let param = self.bind_param(value_as_text(value));
                Ok(format!("{column} {} {param}", operator.symbol()))
            }
            BuiltinField::Kind if self.signal == Signal::Metrics => {
                ensure_equality_operator(operator)?;
                let param = self.bind_param(value_as_text(value));
                Ok(format!("{column} {} {param}", operator.symbol()))
            }
            BuiltinField::Level => compile_level_comparison(&column, operator, value),
            BuiltinField::TraceId | BuiltinField::SpanId => {
                ensure_equality_operator(operator)?;
                let id = match value {
                    Value::String(hex) if builtin_field == BuiltinField::TraceId => {
                        TraceId::parse_hex(hex)
                            .map(|id| id.0.to_vec())
                            .map_err(|error| InvalidQuery(error.to_string()))?
                    }
                    Value::String(hex) => SpanId::parse_hex(hex)
                        .map(|id| id.0.to_vec())
                        .map_err(|error| InvalidQuery(error.to_string()))?,
                    _ => return invalid_query(format!("{name} takes hex digits, not {value}")),
                };
                let param = self.bind_param(id);
                Ok(format!("{column} {} {param}", operator.symbol()))
            }
            BuiltinField::Kind | BuiltinField::Status => {
                ensure_equality_operator(operator)?;
                let fixed_values = builtin_field.fixed_values(self.signal);
                let not_a_fixed_value = |written: &dyn fmt::Display| {
                    InvalidQuery(format!(
                        "{name} is one of {}, not {written}",
                        fixed_values.join(", ")
                    ))
                };
                let number = match value {
                    Value::Int(number) => *number,
                    Value::String(word) => {
                        find_kind_or_status_number(builtin_field, fixed_values, word)
                            .ok_or_else(|| not_a_fixed_value(word))?
                    }
                    _ => return Err(not_a_fixed_value(value)),
                };
                Ok(format!("{column} {} {number}", operator.symbol()))
            }
            BuiltinField::Error | BuiltinField::Root => {
                ensure_equality_operator(operator)?;
                let &Value::Bool(wanted) = value else {
                    return invalid_query(format!("{name} is true or false, not {value}"));
                };
                let wanted = wanted != (operator == Operator::Ne);
                Ok(match (builtin_field, wanted) {
                    (BuiltinField::Error, true) => {
                        format!("{column} = {}", SpanStatus::Error.number())
                    }
                    (BuiltinField::Error, false) => {
                        format!("{column} != {}", SpanStatus::Error.number())
                    }
                    (_, true) => format!("{column} IS NULL"),
                    (_, false) => format!("{column} IS NOT NULL"),
                })
            }
            BuiltinField::Duration => {
                let duration_ns = match value {
                    Value::Duration(duration_ns) | Value::Int(duration_ns) => *duration_ns,
                    _ => {
                        return invalid_query(format!(
                            "duration takes a duration such as 500ms, not {value}"
                        ));
                    }
                };
                let param = self.bind_param(duration_ns);
                Ok(format!("{column} {} {param}", operator.symbol()))
            }
        }
    }
}

fn find_kind_or_status_number(
    builtin_field: BuiltinField,
    fixed_values: &[&str],
    word: &str,
) -> Option<i64> {
    let name = fixed_values
        .iter()
        .find(|fixed_value| fixed_value.eq_ignore_ascii_case(word))?;
    let number = match builtin_field {
        BuiltinField::Kind => SpanKind::from_name(name)?.number(),
        _ => SpanStatus::from_name(name)?.number(),
    };
    Some(i64::from(number))
}

fn compile_level_comparison(column: &str, operator: Operator, value: &Value) -> Result<String> {
    let (low, high) = match value {
        Value::Int(number) => {
            let number = i32::try_from(*number)
                .map_err(|_| InvalidQuery(format!("{number} is not a severity")))?;
            (number, number)
        }
        Value::String(level) => Severity::parse_level_or_number(level)
            .map_err(|error| InvalidQuery(error.to_string()))?
            .level_number_range()
            .into_inner(),
        _ => return invalid_query(format!("level takes a level such as warn, not {value}")),
    };
    Ok(match operator {
        Operator::Eq => format!("{column} BETWEEN {low} AND {high}"),
        Operator::Ne => format!("{column} NOT BETWEEN {low} AND {high}"),
        Operator::Lt => format!("{column} < {low}"),
        Operator::Le => format!("{column} <= {high}"),
        Operator::Gt => format!("{column} > {high}"),
        Operator::Ge => format!("{column} >= {low}"),
    })
}

fn value_as_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn value_as_scalar(value: &Value) -> SqliteValue {
    match value {
        Value::String(text) => SqliteValue::Text(text.clone()),
        Value::Int(integer) | Value::Duration(integer) => SqliteValue::Integer(*integer),
        Value::Float(real) => SqliteValue::Real(*real),
        Value::Bool(flag) => SqliteValue::Integer(i64::from(*flag)),
    }
}

fn equal_sql_values(value: &Value) -> Vec<SqliteValue> {
    // Apps send a number both as a number and as a string.
    match value {
        Value::String(text) => {
            let mut values = vec![SqliteValue::Text(text.clone())];
            if let Ok(integer) = text.parse::<i64>() {
                values.push(SqliteValue::Integer(integer));
            } else if let Ok(real) = text.parse::<f64>() {
                values.push(SqliteValue::Real(real));
            }
            values
        }
        Value::Int(integer) | Value::Duration(integer) => vec![
            SqliteValue::Integer(*integer),
            SqliteValue::Text(integer.to_string()),
        ],
        Value::Float(real) => vec![
            SqliteValue::Real(*real),
            SqliteValue::Text(real.to_string()),
        ],
        Value::Bool(flag) => vec![SqliteValue::Integer(i64::from(*flag))],
    }
}
