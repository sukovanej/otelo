use std::collections::BTreeSet;
use std::fmt;

use otelo_query::{BuiltinField, Expression, Field, Operator, Query, Signal, Value};
use rusqlite::types::Value as SqliteValue;

use otelo_indexed_storage::{
    IndexedAttribute, IndexedSignal, Severity, SpanId, SpanKind, SpanStatus, TraceId,
};

use super::WhereClause;
use super::stored_attributes::{StoredAttributes, format_text_value_json};
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
pub struct EncodedRecordAliases {
    pub record: &'static str,
    pub stable_attribute_set: &'static str,
    pub record_group: &'static str,
    pub resource: &'static str,
}

#[derive(Clone, Copy)]
pub enum TableAliases {
    EncodedRecords(EncodedRecordAliases),
    MetricSeries {
        record: &'static str,
        resource: &'static str,
    },
}

impl TableAliases {
    pub const fn record(self) -> &'static str {
        match self {
            Self::EncodedRecords(aliases) => aliases.record,
            Self::MetricSeries { record, .. } => record,
        }
    }

    pub const fn resource(self) -> &'static str {
        match self {
            Self::EncodedRecords(aliases) => aliases.resource,
            Self::MetricSeries { resource, .. } => resource,
        }
    }
}

pub fn builtin_column(aliases: TableAliases, builtin_field: BuiltinField) -> String {
    let (record, resource) = (aliases.record(), aliases.resource());
    match builtin_field {
        BuiltinField::Service => format!("{resource}.service"),
        BuiltinField::Level => format!("{record}.severity_number"),
        BuiltinField::Body => format!("{record}.body"),
        BuiltinField::TraceId => format!("{record}.trace_id"),
        BuiltinField::SpanId => format!("{record}.span_id"),
        BuiltinField::Name => match aliases {
            TableAliases::EncodedRecords(aliases) => format!("{}.name", aliases.record_group),
            TableAliases::MetricSeries { .. } => format!("{record}.name"),
        },
        BuiltinField::Kind => format!("{record}.kind"),
        BuiltinField::Status | BuiltinField::Error => format!("{record}.status_code"),
        BuiltinField::Duration => format!("{record}.duration_ns"),
        BuiltinField::Root => format!("{record}.parent_span_id"),
        BuiltinField::Unit => format!("{record}.unit"),
    }
}

pub struct QueryContext<'a> {
    pub aliases: TableAliases,
    pub indexed_attributes: &'a BTreeSet<IndexedAttribute>,
    pub stored_attributes: &'a StoredAttributes,
}

pub fn compile_query(
    query: &Query,
    context: &QueryContext,
    param_prefix: &str,
    where_clause: &mut WhereClause,
) -> Result<Vec<String>> {
    let Some(expression) = &query.expression else {
        return Ok(Vec::new());
    };
    let mut compiler = Compiler {
        signal: query.signal,
        aliases: context.aliases,
        stored_attributes: context.stored_attributes,
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
                // A stable key is found through the index of the stable sets.
                let is_indexed = IndexedAttribute::new(signal, key)
                    .is_ok_and(|attribute| context.indexed_attributes.contains(&attribute))
                    || context
                        .stored_attributes
                        .stored_key(key)
                        .encodings
                        .is_only_stable();
                if !is_indexed && !unindexed.contains(key) {
                    unindexed.push(key.clone());
                }
            }
        }
    }
    Ok(unindexed)
}

enum AttributePredicate {
    EqualsAny(Vec<SqliteValue>),
    Compare(Operator, SqliteValue),
    Contains(String),
}

struct Compiler<'a> {
    signal: Signal,
    aliases: TableAliases,
    stored_attributes: &'a StoredAttributes,
    param_prefix: &'a str,
    where_clause: &'a mut WhereClause,
    next_param: usize,
}

impl Compiler<'_> {
    // A branch binds its own parameters, since SQLite refuses a parameter the statement lacks.
    fn compile_predicate_on(
        &mut self,
        predicate: &AttributePredicate,
        extracted_sql: &str,
    ) -> String {
        match predicate {
            AttributePredicate::EqualsAny(values) => {
                let params: Vec<String> = values
                    .iter()
                    .map(|value| self.bind_param(value.clone()))
                    .collect();
                match params.as_slice() {
                    [param] => format!("{extracted_sql} = {param}"),
                    params => format!("{extracted_sql} IN ({})", params.join(", ")),
                }
            }
            AttributePredicate::Compare(operator, value) => {
                let param = self.bind_param(value.clone());
                format!("{extracted_sql} {} {param}", operator.symbol())
            }
            AttributePredicate::Contains(text) => {
                let param = self.bind_param(text.clone());
                format!("instr({extracted_sql}, {param}) > 0")
            }
        }
    }

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
                Field::Attribute(key)
                    if let TableAliases::EncodedRecords(aliases) = self.aliases =>
                {
                    self.compile_encoded_has(aliases, key)?
                }
                _ => format!("json_type({}) IS NOT NULL", self.json_extract_args(field)?),
            },
        })
    }

    const fn find_encoded_record_attribute<'f>(
        &self,
        field: &'f Field,
    ) -> Option<(EncodedRecordAliases, &'f str)> {
        match (field, self.aliases) {
            (Field::Attribute(key), TableAliases::EncodedRecords(aliases)) => {
                Some((aliases, key.as_str()))
            }
            _ => None,
        }
    }

    fn compile_attribute_predicate(
        &mut self,
        field: &Field,
        predicate: &AttributePredicate,
        compared_texts: &[String],
    ) -> Result<String> {
        let Some((aliases, key)) = self.find_encoded_record_attribute(field) else {
            let extracted_sql = format!("json_extract({})", self.json_extract_args(field)?);
            return Ok(self.compile_predicate_on(predicate, &extracted_sql));
        };
        let key_path = build_attribute_json_path(key)?;
        let stored_key = self.stored_attributes.stored_key(key);
        let record = aliases.record;
        let mut branches = Vec::new();
        if stored_key.encodings.has_stable_values {
            let condition = self.compile_predicate_on(
                predicate,
                &format!("json_extract(filtered_stable_attribute_set.attributes, {key_path})"),
            );
            branches.push(format!(
                "{record}.stable_attribute_set_id IN (
                   SELECT filtered_stable_attribute_set.id
                   FROM stable_attribute_sets filtered_stable_attribute_set
                   WHERE {condition})"
            ));
        }
        if let Some(key_id) = stored_key.key_id {
            let id_path = key_id.json_path();
            let interned_sql = format!("json_extract({record}.interned_attributes, {id_path})");
            if stored_key.encodings.has_interned_values {
                match predicate {
                    // The ids of the values let SQLite use the index of the attribute.
                    AttributePredicate::EqualsAny(_) => {
                        let value_ids: Vec<i64> = compared_texts
                            .iter()
                            .filter_map(|text| {
                                self.stored_attributes
                                    .find_interned_value_id(&format_text_value_json(text))
                            })
                            .map(|value_id| value_id.0)
                            .collect();
                        if !value_ids.is_empty() {
                            let values = value_ids.into_iter().map(SqliteValue::Integer).collect();
                            branches.push(self.compile_predicate_on(
                                &AttributePredicate::EqualsAny(values),
                                &interned_sql,
                            ));
                        }
                    }
                    AttributePredicate::Compare(..) | AttributePredicate::Contains(_) => {
                        let condition = self.compile_predicate_on(
                            predicate,
                            "json_extract(filtered_interned_attribute_value.value, '$')",
                        );
                        branches.push(format!(
                            "{interned_sql} IN (
                               SELECT filtered_interned_attribute_value.id
                               FROM interned_attribute_values filtered_interned_attribute_value
                               WHERE {condition})"
                        ));
                    }
                }
            }
            if stored_key.encodings.has_literal_values {
                branches.push(self.compile_predicate_on(
                    predicate,
                    &format!("json_extract({record}.literal_attributes, {id_path})"),
                ));
            }
        }
        Ok(join_branches(&branches))
    }

    fn compile_encoded_has(&self, aliases: EncodedRecordAliases, key: &str) -> Result<String> {
        let key_path = build_attribute_json_path(key)?;
        let stored_key = self.stored_attributes.stored_key(key);
        let record = aliases.record;
        let mut branches = Vec::new();
        if stored_key.encodings.has_stable_values {
            branches.push(format!(
                "{record}.stable_attribute_set_id IN (
                   SELECT filtered_stable_attribute_set.id
                   FROM stable_attribute_sets filtered_stable_attribute_set
                   WHERE json_type(filtered_stable_attribute_set.attributes, {key_path}) IS NOT NULL)"
            ));
        }
        if let Some(key_id) = stored_key.key_id {
            let id_path = key_id.json_path();
            if stored_key.encodings.has_interned_values {
                branches.push(format!(
                    "json_extract({record}.interned_attributes, {id_path}) IS NOT NULL"
                ));
            }
            if stored_key.encodings.has_literal_values {
                branches.push(format!(
                    "json_type({record}.literal_attributes, {id_path}) IS NOT NULL"
                ));
            }
        }
        Ok(join_branches(&branches))
    }

    fn compile_joined_terms(&mut self, terms: &[Expression], separator: &str) -> Result<String> {
        let terms = terms
            .iter()
            .map(|term| Ok(format!("({})", self.compile_expression(term)?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(terms.join(separator))
    }

    fn json_extract_args(&self, field: &Field) -> Result<String> {
        let (alias, key) = match field {
            Field::Attribute(key) => (self.aliases.record(), key),
            Field::Resource(key) => (self.aliases.resource(), key),
            Field::Builtin(_) => unreachable!("a built-in field is a column"),
        };
        Ok(format!(
            "{alias}.attributes, {}",
            build_attribute_json_path(key)?
        ))
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
        match operator {
            Operator::Eq => self.compile_equals_any(field, &equal_sql_values(value)),
            // A record without the attribute is not equal to the value either.
            Operator::Ne => Ok(format!(
                "NOT coalesce({}, FALSE)",
                self.compile_equals_any(field, &equal_sql_values(value))?
            )),
            _ => self.compile_attribute_predicate(
                field,
                &AttributePredicate::Compare(operator, value_as_scalar(value)),
                &[],
            ),
        }
    }

    fn compile_equals_any(&mut self, field: &Field, values: &[SqliteValue]) -> Result<String> {
        let compared_texts: Vec<String> = values
            .iter()
            .filter_map(|value| match value {
                SqliteValue::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        self.compile_attribute_predicate(
            field,
            &AttributePredicate::EqualsAny(values.to_vec()),
            &compared_texts,
        )
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
        let sql_values: Vec<SqliteValue> = values.iter().flat_map(equal_sql_values).collect();
        self.compile_equals_any(field, &sql_values)
    }

    fn compile_contains(&mut self, field: &Field, text: &str) -> Result<String> {
        let column = match field {
            Field::Attribute(_) | Field::Resource(_) => {
                return self.compile_attribute_predicate(
                    field,
                    &AttributePredicate::Contains(text.to_owned()),
                    &[],
                );
            }
            Field::Builtin(BuiltinField::Body) => {
                let Some(words) = super::logs::quote_fts_words(text) else {
                    return Ok("TRUE".into());
                };
                let param = self.bind_param(words);
                return Ok(format!(
                    "{}.rowid IN (SELECT rowid FROM log_body_search WHERE log_body_search MATCH {param})",
                    self.aliases.record()
                ));
            }
            Field::Builtin(builtin_field) if builtin_field.is_text() => {
                builtin_column(self.aliases, *builtin_field)
            }
            Field::Builtin(builtin_field) => {
                return invalid_query(format!(
                    "~ takes a text field, and {} is not one",
                    builtin_field.name()
                ));
            }
        };
        let param = self.bind_param(text.to_owned());
        Ok(format!("instr({column}, {param}) > 0"))
    }

    fn compile_builtin_comparison(
        &mut self,
        builtin_field: BuiltinField,
        operator: Operator,
        value: &Value,
    ) -> Result<String> {
        let column = builtin_column(self.aliases, builtin_field);
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
            BuiltinField::Level => self.compile_level_comparison(&column, operator, value),
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
                let param = self.bind_param(number);
                Ok(format!("{column} {} {param}", operator.symbol()))
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

    fn compile_level_comparison(
        &mut self,
        column: &str,
        operator: Operator,
        value: &Value,
    ) -> Result<String> {
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
            Operator::Eq => {
                let (low, high) = (self.bind_param(low), self.bind_param(high));
                format!("{column} BETWEEN {low} AND {high}")
            }
            Operator::Ne => {
                let (low, high) = (self.bind_param(low), self.bind_param(high));
                format!("{column} NOT BETWEEN {low} AND {high}")
            }
            Operator::Lt => format!("{column} < {}", self.bind_param(low)),
            Operator::Le => format!("{column} <= {}", self.bind_param(high)),
            Operator::Gt => format!("{column} > {}", self.bind_param(high)),
            Operator::Ge => format!("{column} >= {}", self.bind_param(low)),
        })
    }
}

fn build_attribute_json_path(key: &str) -> Result<String> {
    if key.contains('"') {
        return invalid_query(format!(
            "the key {key:?} has a double quote, which a query cannot read"
        ));
    }
    // The expression of the attribute's index, so SQLite uses the index when there is one.
    Ok(attribute_json_path(key))
}

fn join_branches(branches: &[String]) -> String {
    match branches {
        [] => "FALSE".into(),
        [branch] => branch.clone(),
        branches => format!("({})", branches.join(" OR ")),
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

pub fn equal_sql_values(value: &Value) -> Vec<SqliteValue> {
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
