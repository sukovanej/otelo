use std::fmt;

use axum::extract::{Query, State};
use otelo_query::{FieldHelp, FieldOrigin, Signal, SuggestionKind, ValueType};
use otelo_storage::query::AttributeKeys;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::Api;
use crate::error::{ApiResult, ErrorBody};
use crate::params::parse_signal;

/// The kind of record a query reads, as the API names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = Signal)]
pub enum SignalName {
    Logs,
    Spans,
    Metrics,
}

impl From<Signal> for SignalName {
    fn from(signal: Signal) -> Self {
        match signal {
            Signal::Logs => Self::Logs,
            Signal::Spans => Self::Spans,
            Signal::Metrics => Self::Metrics,
        }
    }
}

impl fmt::Display for SignalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let signal = match self {
            Self::Logs => Signal::Logs,
            Self::Spans => Signal::Spans,
            Self::Metrics => Signal::Metrics,
        };
        f.write_str(signal.name())
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SignalParams {
    // A string, so a wrong signal gets the JSON error of `parse_signal`.
    #[param(value_type = SignalName)]
    signal: String,
}

/// The attribute keys of the records of a signal and of their resources over
/// the retention, the most common first, with their types.
#[utoipa::path(
    get,
    path = "/api/attributes",
    params(SignalParams),
    responses(
        (status = 200, body = AttributeKeys),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn list_attribute_keys(
    State(api): State<Api>,
    Query(params): Query<SignalParams>,
) -> ApiResult<AttributeKeys> {
    let signal = parse_signal(&params.signal)?;
    api.run_retention_query(move |opened| Ok(opened.queries.list_attribute_keys(signal)?))
        .await
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CompleteParams {
    #[param(value_type = SignalName)]
    signal: String,
    /// The query as typed so far.
    #[serde(rename = "q")]
    #[param(rename = "q")]
    query: Option<String>,
    /// The position of the cursor in the query, in characters. The end of the
    /// query when missing.
    cursor: Option<usize>,
}

/// What can go at the cursor of a query, and what the field there holds.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Completions {
    pub suggestions: Vec<SuggestionBody>,
    /// The field of the term the cursor is in. Missing when the cursor is in
    /// no term, and for an attribute that no record has.
    #[schema(required = true)]
    pub field: Option<FieldBody>,
}

/// A field of a query: where it comes from, its type, and its values.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FieldBody {
    /// The field as a query writes it.
    pub name: String,
    #[serde(flatten)]
    pub origin: FieldOriginBody,
    /// The type of the values. Of an attribute: `string`, `int`, `float`,
    /// `bool`, `array`, `object`, or `mixed`. Of a built-in field: `string`,
    /// `bool`, or `duration`.
    #[serde(rename = "type")]
    pub value_type: ValueType,
    /// The values, the most common first, and 10 at most. For a built-in
    /// field with fixed values, those, in their order.
    pub values: Vec<FieldValueBody>,
    /// How many distinct values the daemon knows.
    pub distinct_values: usize,
    /// Whether the field has more distinct values than the daemon keeps, so
    /// `distinct_values` counts only some of them.
    pub many_values: bool,
}

/// Where a field comes from: the query language, the attributes of the
/// records, or the attributes of their resources.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum FieldOriginBody {
    /// A field of the query language, which every record has.
    Builtin {
        /// What the field holds.
        description: String,
    },
    /// An attribute of the records.
    Attribute {
        /// How many records have the attribute.
        #[serde(rename = "count")]
        record_count: u64,
    },
    /// An attribute of the resources of the records.
    Resource {
        /// How many resources have the attribute.
        #[serde(rename = "count")]
        resource_count: u64,
    },
}

impl From<FieldOrigin> for FieldOriginBody {
    fn from(origin: FieldOrigin) -> Self {
        match origin {
            FieldOrigin::Builtin { description } => Self::Builtin {
                description: description.to_owned(),
            },
            FieldOrigin::Attribute { record_count } => Self::Attribute { record_count },
            FieldOrigin::Resource { resource_count } => Self::Resource { resource_count },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FieldValueBody {
    /// The value as a query writes it.
    pub text: String,
    /// How many records have the value. Missing for the fixed values of a
    /// built-in field.
    #[schema(required = true)]
    pub count: Option<u64>,
}

impl From<FieldHelp> for FieldBody {
    fn from(help: FieldHelp) -> Self {
        Self {
            name: help.name,
            origin: help.origin.into(),
            value_type: help.value_type,
            values: help
                .most_common_values
                .into_iter()
                .map(|value| FieldValueBody {
                    text: value.text,
                    count: value.record_count,
                })
                .collect(),
            distinct_values: help.distinct_value_count,
            many_values: help.has_more_values_than_listed,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SuggestionBody {
    /// The text to put in place of the characters from `start` to `end`.
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub kind: CompletionKind,
    /// The type of a field and how many records have it, or how many records
    /// have a value.
    #[schema(required = true)]
    pub detail: Option<String>,
}

/// What the text of a suggestion is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CompletionKind {
    Field,
    Operator,
    Value,
    Keyword,
}

impl From<SuggestionKind> for CompletionKind {
    fn from(kind: SuggestionKind) -> Self {
        match kind {
            SuggestionKind::Field => Self::Field,
            SuggestionKind::Operator => Self::Operator,
            SuggestionKind::Value => Self::Value,
            SuggestionKind::Keyword => Self::Keyword,
        }
    }
}

impl fmt::Display for CompletionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Field => SuggestionKind::Field,
            Self::Operator => SuggestionKind::Operator,
            Self::Value => SuggestionKind::Value,
            Self::Keyword => SuggestionKind::Keyword,
        };
        f.write_str(kind.name())
    }
}

/// Suggests the fields, operators, values, and keywords that can go at the
/// cursor of a query, from the attributes and values of the retention, and
/// describes the field of the term the cursor is in.
#[utoipa::path(
    get,
    path = "/api/complete",
    params(CompleteParams),
    responses(
        (status = 200, body = Completions),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn complete_query(
    State(api): State<Api>,
    Query(params): Query<CompleteParams>,
) -> ApiResult<Completions> {
    let signal = parse_signal(&params.signal)?;
    let query_text = params.query.unwrap_or_default();
    let cursor_byte_offset = params
        .cursor
        .map_or(query_text.len(), |cursor_char_offset| {
            query_text
                .char_indices()
                .nth(cursor_char_offset)
                .map_or(query_text.len(), |(byte_offset, _)| byte_offset)
        });
    api.run_retention_query(move |opened| {
        let count_chars_before = |byte_offset: usize| query_text[..byte_offset].chars().count();
        let completion =
            otelo_query::complete_query(&query_text, cursor_byte_offset, signal, &*opened.queries);
        let suggestions = completion
            .suggestions
            .into_iter()
            .map(|suggestion| SuggestionBody {
                start: count_chars_before(suggestion.replaced_byte_range.start),
                end: count_chars_before(suggestion.replaced_byte_range.end),
                text: suggestion.text,
                kind: suggestion.kind.into(),
                detail: suggestion.detail,
            })
            .collect();
        Ok(Completions {
            suggestions,
            field: completion.help_for_field_at_cursor.map(FieldBody::from),
        })
    })
    .await
}
