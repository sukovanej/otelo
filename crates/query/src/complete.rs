use std::ops::Range;

use crate::context::{find_context_of_cursor, join_with_and};
use crate::lexer::{Token, TokenType, is_keyword, lex_tokens, needs_no_backticks};
use crate::parser::resolve_field;
use crate::{BuiltinField, Expression, Field, Operator, Signal, Value, ValueType, quote_string};

const MAX_SUGGESTIONS: usize = 50;

pub const MAX_HELP_VALUES: usize = 10;

pub trait Catalog {
    // Both lists come most common first, since completion keeps only the first MAX_SUGGESTIONS.
    // They hold only what the records that match the context have.
    fn keys(&self, signal: Signal, resource: bool, context: Option<&Expression>) -> Vec<KeyInfo>;

    // The values may be only the ones that start with the prefix, ignoring case.
    fn values(
        &self,
        signal: Signal,
        field: &Field,
        lowercase_value_prefix: &str,
        context: Option<&Expression>,
    ) -> FieldValues;

    // Asked only of a built-in field that some records lack.
    fn has_builtin_field(
        &self,
        signal: Signal,
        builtin_field: BuiltinField,
        context: Option<&Expression>,
    ) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyInfo {
    pub key: String,
    pub value_type: ValueType,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValueInfo {
    pub value: Value,
    pub count: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldValues {
    pub listed: Vec<ValueInfo>,
    pub has_more_values_than_listed: bool,
}

pub struct NoCatalog;

impl Catalog for NoCatalog {
    fn keys(&self, _: Signal, _: bool, _: Option<&Expression>) -> Vec<KeyInfo> {
        Vec::new()
    }

    fn values(&self, _: Signal, _: &Field, _: &str, _: Option<&Expression>) -> FieldValues {
        FieldValues::default()
    }

    fn has_builtin_field(&self, _: Signal, _: BuiltinField, _: Option<&Expression>) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionKind {
    Field,
    Operator,
    Value,
    Keyword,
}

impl SuggestionKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Operator => "operator",
            Self::Value => "value",
            Self::Keyword => "keyword",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub text: String,
    pub replaced_byte_range: Range<usize>,
    pub kind: SuggestionKind,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Completion {
    pub suggestions: Vec<Suggestion>,
    pub help_for_field_at_cursor: Option<FieldHelp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldHelp {
    pub name: String,
    pub value_type: ValueType,
    pub origin: FieldOrigin,
    pub most_common_values: Vec<HelpValue>,
    pub distinct_value_count: usize,
    pub has_more_values_than_listed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldOrigin {
    Builtin { description: &'static str },
    Attribute { record_count: u64 },
    Resource { resource_count: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpValue {
    pub text: String,
    // The fixed values of a built-in field are not counted.
    pub record_count: Option<u64>,
}

#[derive(Clone, Debug)]
enum ExpectedNext {
    Term,
    Operator(Field),
    Value(Field),
    InOpen,
    InValue(Field),
    InNext,
    HasOpen,
    HasField,
    HasClose,
    AfterTerm,
}

#[must_use]
pub fn complete_query(
    input: &str,
    cursor: usize,
    signal: Signal,
    catalog: &dyn Catalog,
    outer_context: Option<&Expression>,
) -> Completion {
    let cursor = floor_char_boundary(input, cursor);
    let tokens = lex_tokens(&input[..cursor]);
    let (tokens_before_word, word_at_cursor) = match tokens.last() {
        Some(last) if last.byte_range.end == cursor && can_extend_token(&last.token_type) => {
            (&tokens[..tokens.len() - 1], Some(last))
        }
        _ => (&tokens[..], None),
    };
    let Some((expected, open_parens)) = expected_after_tokens(tokens_before_word, signal) else {
        return Completion::default();
    };
    let context = join_with_and(
        outer_context
            .cloned()
            .into_iter()
            .chain(find_context_of_cursor(input, cursor, signal))
            .collect(),
    );
    let context = context.as_ref();
    let field_at_cursor = field_of_term_at_cursor(&expected, word_at_cursor, signal);
    let known_field = field_at_cursor
        .as_ref()
        .and_then(|field| find_known_field(field, signal, catalog, context));
    let replaced_byte_range = byte_range_of_whole_token_at_cursor(input, word_at_cursor, cursor);
    let prefix = word_at_cursor.map_or(String::new(), |token| match &token.token_type {
        TokenType::Quoted { text, .. } | TokenType::Backticked { text, .. } => text.clone(),
        _ => input[token.byte_range.clone()].to_owned(),
    });
    let lowercase_prefix = prefix.to_lowercase();
    let completes_value = matches!(expected, ExpectedNext::Value(_) | ExpectedNext::InValue(_));
    let values = match &field_at_cursor {
        Some(field) if completes_value => catalog.values(signal, field, &lowercase_prefix, context),
        Some(field) if known_field.is_some() => catalog.values(signal, field, "", context),
        _ => FieldValues::default(),
    };
    let mut matching = MatchingSuggestions {
        suggestions: Vec::new(),
        lowercase_prefix,
        replaced_byte_range,
    };
    match expected {
        ExpectedNext::Term => {
            matching.push_fields(signal, catalog, context, true);
            matching.push_keyword("not");
            matching.push_keyword("has(");
            matching.push_keyword("(");
        }
        ExpectedNext::Operator(field) => {
            matching.push_operators(
                &field,
                known_field.as_ref().map(|(value_type, _)| *value_type),
            );
        }
        ExpectedNext::Value(field) => matching.push_values(signal, &field, &values.listed, &[]),
        ExpectedNext::InValue(field) => {
            let lowercase_written_values = list_values_written_in_open_list(tokens_before_word);
            matching.push_values(signal, &field, &values.listed, &lowercase_written_values);
        }
        ExpectedNext::InOpen | ExpectedNext::HasOpen => matching.push_keyword("("),
        ExpectedNext::InNext => {
            matching.push_keyword(",");
            matching.push_keyword(")");
        }
        // The query compiler rejects has() of a built-in field.
        ExpectedNext::HasField => matching.push_fields(signal, catalog, context, false),
        ExpectedNext::HasClose => matching.push_keyword(")"),
        ExpectedNext::AfterTerm => {
            matching.push_keyword("and");
            matching.push_keyword("or");
            if open_parens > 0 {
                matching.push_keyword(")");
            }
            if !matching.lowercase_prefix.is_empty() {
                matching.push_fields(signal, catalog, context, true);
            }
        }
    }
    matching.suggestions.truncate(MAX_SUGGESTIONS);
    Completion {
        suggestions: matching.suggestions,
        help_for_field_at_cursor: field_at_cursor.zip(known_field).map(
            |(field, (value_type, origin))| {
                describe_field(&field, value_type, origin, signal, &values)
            },
        ),
    }
}

fn byte_range_of_whole_token_at_cursor(
    input: &str,
    word_at_cursor: Option<&Token>,
    cursor: usize,
) -> Range<usize> {
    word_at_cursor.map_or(cursor..cursor, |token| {
        let end = lex_tokens(input)
            .into_iter()
            .find(|whole_token| whole_token.byte_range.start == token.byte_range.start)
            .map_or(cursor, |whole_token| whole_token.byte_range.end);
        token.byte_range.start..end.max(cursor)
    })
}

fn field_of_term_at_cursor(
    expected: &ExpectedNext,
    word_at_cursor: Option<&Token>,
    signal: Signal,
) -> Option<Field> {
    match expected {
        ExpectedNext::Operator(field)
        | ExpectedNext::Value(field)
        | ExpectedNext::InValue(field) => Some(field.clone()),
        ExpectedNext::Term | ExpectedNext::AfterTerm | ExpectedNext::HasField => {
            match &word_at_cursor?.token_type {
                TokenType::Word(word) if !is_keyword(word) => Some(resolve_field(signal, word)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn find_known_field(
    field: &Field,
    signal: Signal,
    catalog: &dyn Catalog,
    context: Option<&Expression>,
) -> Option<(ValueType, FieldOrigin)> {
    let find_key = |wanted: &str, resource: bool| {
        catalog
            .keys(signal, resource, context)
            .into_iter()
            .find(|info| info.key == wanted)
    };
    match field {
        Field::Builtin(builtin_field) => Some((
            builtin_field.value_type(),
            FieldOrigin::Builtin {
                description: builtin_field.description(signal),
            },
        )),
        Field::Attribute(key) => find_key(key, false).map(|info| {
            (
                info.value_type,
                FieldOrigin::Attribute {
                    record_count: info.count,
                },
            )
        }),
        Field::Resource(key) => find_key(key, true).map(|info| {
            (
                info.value_type,
                FieldOrigin::Resource {
                    resource_count: info.count,
                },
            )
        }),
    }
}

fn describe_field(
    field: &Field,
    value_type: ValueType,
    origin: FieldOrigin,
    signal: Signal,
    values: &FieldValues,
) -> FieldHelp {
    let fixed_values = match field {
        Field::Builtin(builtin_field) => builtin_field.fixed_values(signal),
        _ => &[],
    };
    let mut all_values: Vec<HelpValue> = fixed_values
        .iter()
        .map(|value| HelpValue {
            text: (*value).to_owned(),
            record_count: None,
        })
        .collect();
    all_values.extend(values.listed.iter().map(|info| HelpValue {
        text: value_as_written(&info.value),
        record_count: Some(info.count),
    }));
    let distinct_value_count = all_values.len();
    all_values.truncate(MAX_HELP_VALUES);
    FieldHelp {
        name: field.to_string(),
        value_type,
        origin,
        most_common_values: all_values,
        distinct_value_count,
        has_more_values_than_listed: values.has_more_values_than_listed,
    }
}

fn value_as_written(value: &Value) -> String {
    match value {
        Value::String(text) => quote_string(text),
        value => value.to_string(),
    }
}

const fn can_extend_token(token_type: &TokenType) -> bool {
    matches!(
        token_type,
        TokenType::Word(_)
            | TokenType::Int(_)
            | TokenType::Float(_)
            | TokenType::DurationNanos(_)
            | TokenType::Quoted { closed: false, .. }
            | TokenType::Backticked { closed: false, .. }
    )
}

fn floor_char_boundary(input: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(input.len());
    while !input.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

fn expected_after_tokens(tokens: &[Token], signal: Signal) -> Option<(ExpectedNext, usize)> {
    let mut expected = ExpectedNext::Term;
    let mut open_parens = 0_usize;
    let mut index = 0;
    while index < tokens.len() {
        let token_type = &tokens[index].token_type;
        let word = match token_type {
            TokenType::Word(word) => Some(word.to_ascii_lowercase()),
            _ => None,
        };
        expected = match (expected, token_type) {
            (ExpectedNext::AfterTerm, TokenType::Word(_))
                if matches!(word.as_deref(), Some("and" | "or")) =>
            {
                ExpectedNext::Term
            }
            (ExpectedNext::AfterTerm, TokenType::CloseParen) if open_parens > 0 => {
                open_parens -= 1;
                ExpectedNext::AfterTerm
            }
            // A term right after another joins it with AND.
            (ExpectedNext::Term, _)
            | (
                ExpectedNext::AfterTerm,
                TokenType::Word(_) | TokenType::Backticked { .. } | TokenType::OpenParen,
            ) => expected_after_term_start(
                token_type,
                word.as_deref(),
                tokens.get(index + 1),
                signal,
                &mut open_parens,
            )?,
            (ExpectedNext::Operator(field), TokenType::Operator(_) | TokenType::Tilde) => {
                ExpectedNext::Value(field)
            }
            (ExpectedNext::Operator(field), TokenType::Word(_))
                if word.as_deref() == Some("in") =>
            {
                index += 1;
                match tokens.get(index).map(|token| &token.token_type) {
                    Some(TokenType::OpenParen) => ExpectedNext::InValue(field),
                    None => ExpectedNext::InOpen,
                    Some(_) => return None,
                }
            }
            (ExpectedNext::Value(_), _) if is_value_token(token_type) => ExpectedNext::AfterTerm,
            (ExpectedNext::InValue(_), _) if is_value_token(token_type) => ExpectedNext::InNext,
            (ExpectedNext::InNext, TokenType::Comma) => {
                let field = field_of_open_in_list(&tokens[..index], signal)?;
                ExpectedNext::InValue(field)
            }
            (ExpectedNext::HasOpen, TokenType::OpenParen) => ExpectedNext::HasField,
            (
                ExpectedNext::HasField,
                TokenType::Word(_) | TokenType::Backticked { closed: true, .. },
            ) => ExpectedNext::HasClose,
            (ExpectedNext::InNext | ExpectedNext::HasClose, TokenType::CloseParen) => {
                ExpectedNext::AfterTerm
            }
            _ => return None,
        };
        index += 1;
    }
    Some((expected, open_parens))
}

fn expected_after_term_start(
    token_type: &TokenType,
    word: Option<&str>,
    next_token: Option<&Token>,
    signal: Signal,
    open_parens: &mut usize,
) -> Option<ExpectedNext> {
    Some(match (token_type, word) {
        (TokenType::OpenParen, _) => {
            *open_parens += 1;
            ExpectedNext::Term
        }
        (_, Some("not")) => ExpectedNext::Term,
        (_, Some("has"))
            if next_token.is_none_or(|token| token.token_type == TokenType::OpenParen) =>
        {
            ExpectedNext::HasOpen
        }
        (TokenType::Word(text), _) if !is_keyword(text) => {
            ExpectedNext::Operator(resolve_field(signal, text))
        }
        (TokenType::Backticked { text, closed: true }, _) => {
            ExpectedNext::Operator(Field::Attribute(text.clone()))
        }
        _ => return None,
    })
}

const fn is_value_token(token_type: &TokenType) -> bool {
    matches!(
        token_type,
        TokenType::Word(_)
            | TokenType::Int(_)
            | TokenType::Float(_)
            | TokenType::DurationNanos(_)
            | TokenType::Quoted { closed: true, .. }
    )
}

fn list_values_written_in_open_list(tokens: &[Token]) -> Vec<String> {
    let open_paren_index = tokens
        .iter()
        .rposition(|token| token.token_type == TokenType::OpenParen);
    open_paren_index
        .map(|index| &tokens[index + 1..])
        .unwrap_or_default()
        .iter()
        .filter_map(|token| value_of_token(&token.token_type))
        .map(|value| lowercase_unquoted_text(&value))
        .collect()
}

fn lowercase_unquoted_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_lowercase(),
        value => value_as_written(value).to_lowercase(),
    }
}

fn value_of_token(token_type: &TokenType) -> Option<Value> {
    Some(match token_type {
        TokenType::Quoted { text, closed: true } => Value::String(text.clone()),
        TokenType::Word(word) if word == "true" => Value::Bool(true),
        TokenType::Word(word) if word == "false" => Value::Bool(false),
        TokenType::Word(word) if !is_keyword(word) => Value::String(word.clone()),
        TokenType::Int(integer) => Value::Int(*integer),
        TokenType::Float(float) => Value::Float(*float),
        TokenType::DurationNanos(nanos) => Value::Duration(*nanos),
        _ => return None,
    })
}

fn field_of_open_in_list(tokens: &[Token], signal: Signal) -> Option<Field> {
    let in_keyword_index = tokens.iter().rposition(|token| {
        matches!(&token.token_type, TokenType::Word(word) if word.eq_ignore_ascii_case("in"))
    })?;
    match &tokens.get(in_keyword_index.checked_sub(1)?)?.token_type {
        TokenType::Word(word) => Some(resolve_field(signal, word)),
        TokenType::Backticked { text, .. } => Some(Field::Attribute(text.clone())),
        _ => None,
    }
}

struct MatchingSuggestions {
    suggestions: Vec<Suggestion>,
    lowercase_prefix: String,
    replaced_byte_range: Range<usize>,
}

impl MatchingSuggestions {
    fn push_suggestion(&mut self, text: String, kind: SuggestionKind, detail: Option<String>) {
        if !text.to_lowercase().starts_with(&self.lowercase_prefix)
            || self
                .suggestions
                .iter()
                .any(|suggestion| suggestion.text == text)
        {
            return;
        }
        self.suggestions.push(Suggestion {
            text,
            replaced_byte_range: self.replaced_byte_range.clone(),
            kind,
            detail,
        });
    }

    fn push_keyword(&mut self, keyword: &str) {
        self.push_suggestion(keyword.into(), SuggestionKind::Keyword, None);
    }

    fn push_fields(
        &mut self,
        signal: Signal,
        catalog: &dyn Catalog,
        context: Option<&Expression>,
        include_builtin_fields: bool,
    ) {
        if include_builtin_fields {
            for &builtin_field in signal.builtin_fields() {
                let can_be_suggested = builtin_field.name().starts_with(&self.lowercase_prefix)
                    && (builtin_field.is_on_every_record(signal)
                        || catalog.has_builtin_field(signal, builtin_field, context));
                if !can_be_suggested {
                    continue;
                }
                self.push_suggestion(
                    builtin_field.name().into(),
                    SuggestionKind::Field,
                    Some(format!("built-in, {}", builtin_field.value_type())),
                );
            }
        }
        for info in catalog.keys(signal, false, context) {
            let detail = describe_key(info.value_type, info.count);
            self.push_suggestion(
                Field::Attribute(info.key).to_string(),
                SuggestionKind::Field,
                Some(detail),
            );
        }
        // A resource key has no backtick form, so one that needs backticks cannot be written.
        for info in catalog.keys(signal, true, context) {
            if needs_no_backticks(&info.key) {
                let detail = describe_key(info.value_type, info.count);
                self.push_suggestion(
                    Field::Resource(info.key).to_string(),
                    SuggestionKind::Field,
                    Some(detail),
                );
            }
        }
    }

    fn push_operators(&mut self, field: &Field, known_value_type: Option<ValueType>) {
        let (is_ordered, is_text) = match (field, known_value_type) {
            (Field::Builtin(builtin_field), _) => {
                (builtin_field.is_ordered(), builtin_field.is_text())
            }
            (_, Some(ValueType::Int | ValueType::Float)) => (true, false),
            (_, Some(ValueType::String)) => (false, true),
            (_, Some(ValueType::Mixed) | None) => (true, true),
            (_, Some(_)) => (false, false),
        };
        let mut operators = vec![Operator::Eq, Operator::Ne];
        if is_ordered {
            operators.extend([Operator::Lt, Operator::Le, Operator::Gt, Operator::Ge]);
        }
        for operator in operators {
            self.push_suggestion(operator.symbol().into(), SuggestionKind::Operator, None);
        }
        if is_text {
            self.push_suggestion("~".into(), SuggestionKind::Operator, None);
        }
        self.push_suggestion("in".into(), SuggestionKind::Operator, None);
    }

    fn push_values(
        &mut self,
        signal: Signal,
        field: &Field,
        listed_values: &[ValueInfo],
        lowercase_written_values: &[String],
    ) {
        if let Field::Builtin(builtin_field) = field {
            for value in builtin_field.fixed_values(signal) {
                if !lowercase_written_values.contains(&value.to_lowercase()) {
                    self.push_suggestion((*value).into(), SuggestionKind::Value, None);
                }
            }
            if matches!(builtin_field, BuiltinField::Duration) {
                return;
            }
        }
        for info in listed_values {
            let text = value_as_written(&info.value);
            let unquoted_text = lowercase_unquoted_text(&info.value);
            if unquoted_text.starts_with(&self.lowercase_prefix)
                && !lowercase_written_values.contains(&unquoted_text)
                && !self
                    .suggestions
                    .iter()
                    .any(|suggestion| suggestion.text == text)
            {
                self.suggestions.push(Suggestion {
                    text,
                    replaced_byte_range: self.replaced_byte_range.clone(),
                    kind: SuggestionKind::Value,
                    detail: Some(info.count.to_string()),
                });
            }
        }
    }
}

fn describe_key(value_type: ValueType, count: u64) -> String {
    format!("{value_type}, {count}")
}
