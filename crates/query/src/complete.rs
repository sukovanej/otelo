use std::ops::Range;

use crate::lexer::{Tok, Token, is_keyword, lex_tokens, needs_no_backticks};
use crate::parser::resolve_field;
use crate::{Builtin, Field, Op, Signal, Value, quote};

const MAX_SUGGESTIONS: usize = 50;

pub const MAX_HELP_VALUES: usize = 10;

pub trait Catalog {
    // Both lists come most common first, since completion keeps only the first MAX_SUGGESTIONS.
    fn keys(&self, signal: Signal, resource: bool) -> Vec<KeyInfo>;

    fn values(&self, signal: Signal, field: &Field) -> FieldValues;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyInfo {
    pub key: String,
    pub kind: String,
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
    // The field has more distinct values than the catalog lists.
    pub many_values: bool,
}

pub struct NoCatalog;

impl Catalog for NoCatalog {
    fn keys(&self, _: Signal, _: bool) -> Vec<KeyInfo> {
        Vec::new()
    }

    fn values(&self, _: Signal, _: &Field) -> FieldValues {
        FieldValues::default()
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
    pub const fn as_str(self) -> &'static str {
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
    pub replace: Range<usize>,
    pub kind: SuggestionKind,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Completion {
    pub suggestions: Vec<Suggestion>,
    pub field_at_cursor: Option<FieldHelp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldHelp {
    pub name: String,
    pub type_name: String,
    pub origin: FieldOrigin,
    pub most_common_values: Vec<HelpValue>,
    pub distinct_value_count: usize,
    pub many_values: bool,
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
pub fn complete(input: &str, cursor: usize, signal: Signal, catalog: &dyn Catalog) -> Completion {
    let cursor = floor_char_boundary(input, cursor);
    let tokens = lex_tokens(&input[..cursor]);
    let (tokens_before_word, word_at_cursor) = match tokens.last() {
        Some(last) if last.byte_range.end == cursor && can_extend_token(&last.tok) => {
            (&tokens[..tokens.len() - 1], Some(last))
        }
        _ => (&tokens[..], None),
    };
    let Some((expect, open_parens)) = expected_after_tokens(tokens_before_word, signal) else {
        return Completion::default();
    };
    let field_at_cursor = field_of_term_at_cursor(&expect, word_at_cursor, signal);
    let known_field = field_at_cursor
        .as_ref()
        .and_then(|field| find_known_field(field, signal, catalog));
    let completes_value = matches!(expect, ExpectedNext::Value(_) | ExpectedNext::InValue(_));
    let values = match &field_at_cursor {
        Some(field) if completes_value || known_field.is_some() => catalog.values(signal, field),
        _ => FieldValues::default(),
    };
    let replace = word_at_cursor.map_or(cursor..cursor, |token| {
        // Also replace the part after the cursor, such as the rest of a string and its closing quote.
        let end = lex_tokens(input)
            .into_iter()
            .find(|full| full.byte_range.start == token.byte_range.start)
            .map_or(cursor, |full| full.byte_range.end);
        token.byte_range.start..end.max(cursor)
    });
    let prefix = word_at_cursor.map_or(String::new(), |token| match &token.tok {
        Tok::Quoted { text, .. } | Tok::Backticked { text, .. } => text.clone(),
        _ => input[token.byte_range.clone()].to_owned(),
    });
    let mut out = Out {
        suggestions: Vec::new(),
        lowercase_prefix: prefix.to_lowercase(),
        replace,
    };
    match expect {
        ExpectedNext::Term => {
            out.push_fields(signal, catalog, true);
            out.push_keyword("not");
            out.push_keyword("has(");
            out.push_keyword("(");
        }
        ExpectedNext::Operator(field) => out.push_operators(&field),
        ExpectedNext::Value(field) | ExpectedNext::InValue(field) => {
            out.push_values(signal, &field, &values.listed);
        }
        ExpectedNext::InOpen | ExpectedNext::HasOpen => out.push_keyword("("),
        ExpectedNext::InNext => {
            out.push_keyword(",");
            out.push_keyword(")");
        }
        // Every record has the built-in fields, so has() only makes sense on an attribute.
        ExpectedNext::HasField => out.push_fields(signal, catalog, false),
        ExpectedNext::HasClose => out.push_keyword(")"),
        ExpectedNext::AfterTerm => {
            out.push_keyword("and");
            out.push_keyword("or");
            if open_parens > 0 {
                out.push_keyword(")");
            }
            if !out.lowercase_prefix.is_empty() {
                out.push_fields(signal, catalog, true);
            }
        }
    }
    out.suggestions.truncate(MAX_SUGGESTIONS);
    Completion {
        suggestions: out.suggestions,
        field_at_cursor: field_at_cursor
            .zip(known_field)
            .map(|(field, (type_name, origin))| {
                describe_field(&field, type_name, origin, signal, &values)
            }),
    }
}

fn field_of_term_at_cursor(
    expect: &ExpectedNext,
    word_at_cursor: Option<&Token>,
    signal: Signal,
) -> Option<Field> {
    match expect {
        ExpectedNext::Operator(field)
        | ExpectedNext::Value(field)
        | ExpectedNext::InValue(field) => Some(field.clone()),
        ExpectedNext::Term | ExpectedNext::AfterTerm | ExpectedNext::HasField => {
            match &word_at_cursor?.tok {
                Tok::Word(word) if !is_keyword(word) => Some(resolve_field(signal, word)),
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
) -> Option<(String, FieldOrigin)> {
    let find_key = |wanted: &str, resource: bool| {
        catalog
            .keys(signal, resource)
            .into_iter()
            .find(|info| info.key == wanted)
    };
    match field {
        Field::Builtin(builtin) => Some((
            builtin.type_name().to_owned(),
            FieldOrigin::Builtin {
                description: builtin.description(signal),
            },
        )),
        Field::Attribute(key) => find_key(key, false).map(|info| {
            (
                info.kind,
                FieldOrigin::Attribute {
                    record_count: info.count,
                },
            )
        }),
        Field::Resource(key) => find_key(key, true).map(|info| {
            (
                info.kind,
                FieldOrigin::Resource {
                    resource_count: info.count,
                },
            )
        }),
    }
}

fn describe_field(
    field: &Field,
    type_name: String,
    origin: FieldOrigin,
    signal: Signal,
    values: &FieldValues,
) -> FieldHelp {
    let fixed_values = match field {
        Field::Builtin(builtin) => builtin.values(signal),
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
        type_name,
        origin,
        most_common_values: all_values,
        distinct_value_count,
        many_values: values.many_values,
    }
}

fn value_as_written(value: &Value) -> String {
    match value {
        Value::String(text) => quote(text),
        value => value.to_string(),
    }
}

const fn can_extend_token(tok: &Tok) -> bool {
    matches!(
        tok,
        Tok::Word(_)
            | Tok::Int(_)
            | Tok::Float(_)
            | Tok::DurationNanos(_)
            | Tok::Quoted { closed: false, .. }
            | Tok::Backticked { closed: false, .. }
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
    let mut expect = ExpectedNext::Term;
    let mut open_parens = 0_usize;
    let mut i = 0;
    while i < tokens.len() {
        let tok = &tokens[i].tok;
        let word = match tok {
            Tok::Word(word) => Some(word.to_ascii_lowercase()),
            _ => None,
        };
        expect = match (expect, tok) {
            (ExpectedNext::AfterTerm, Tok::Word(_))
                if matches!(word.as_deref(), Some("and" | "or")) =>
            {
                ExpectedNext::Term
            }
            (ExpectedNext::AfterTerm, Tok::RParen) if open_parens > 0 => {
                open_parens -= 1;
                ExpectedNext::AfterTerm
            }
            // A term right after another joins it with AND.
            (ExpectedNext::Term, _)
            | (ExpectedNext::AfterTerm, Tok::Word(_) | Tok::Backticked { .. } | Tok::LParen) => {
                expected_after_term_start(
                    tok,
                    word.as_deref(),
                    tokens.get(i + 1),
                    signal,
                    &mut open_parens,
                )?
            }
            (ExpectedNext::Operator(field), Tok::Op(_) | Tok::Tilde) => ExpectedNext::Value(field),
            (ExpectedNext::Operator(field), Tok::Word(_)) if word.as_deref() == Some("in") => {
                i += 1;
                match tokens.get(i).map(|t| &t.tok) {
                    Some(Tok::LParen) => ExpectedNext::InValue(field),
                    None => ExpectedNext::InOpen,
                    Some(_) => return None,
                }
            }
            (ExpectedNext::Value(_), tok) if is_value(tok) => ExpectedNext::AfterTerm,
            (ExpectedNext::InValue(_), tok) if is_value(tok) => ExpectedNext::InNext,
            (ExpectedNext::InNext, Tok::Comma) => {
                let field = field_of_open_in_list(&tokens[..i], signal)?;
                ExpectedNext::InValue(field)
            }
            (ExpectedNext::HasOpen, Tok::LParen) => ExpectedNext::HasField,
            (ExpectedNext::HasField, Tok::Word(_) | Tok::Backticked { closed: true, .. }) => {
                ExpectedNext::HasClose
            }
            (ExpectedNext::InNext | ExpectedNext::HasClose, Tok::RParen) => ExpectedNext::AfterTerm,
            _ => return None,
        };
        i += 1;
    }
    Some((expect, open_parens))
}

fn expected_after_term_start(
    tok: &Tok,
    word: Option<&str>,
    next: Option<&Token>,
    signal: Signal,
    open_parens: &mut usize,
) -> Option<ExpectedNext> {
    Some(match (tok, word) {
        (Tok::LParen, _) => {
            *open_parens += 1;
            ExpectedNext::Term
        }
        (_, Some("not")) => ExpectedNext::Term,
        (_, Some("has")) if next.is_none_or(|t| t.tok == Tok::LParen) => ExpectedNext::HasOpen,
        (Tok::Word(text), _) if !is_keyword(text) => {
            ExpectedNext::Operator(resolve_field(signal, text))
        }
        (Tok::Backticked { text, closed: true }, _) => {
            ExpectedNext::Operator(Field::Attribute(text.clone()))
        }
        _ => return None,
    })
}

const fn is_value(tok: &Tok) -> bool {
    matches!(
        tok,
        Tok::Word(_)
            | Tok::Int(_)
            | Tok::Float(_)
            | Tok::DurationNanos(_)
            | Tok::Quoted { closed: true, .. }
    )
}

fn field_of_open_in_list(tokens: &[Token], signal: Signal) -> Option<Field> {
    let at = tokens
        .iter()
        .rposition(|t| matches!(&t.tok, Tok::Word(word) if word.eq_ignore_ascii_case("in")))?;
    match &tokens.get(at.checked_sub(1)?)?.tok {
        Tok::Word(word) => Some(resolve_field(signal, word)),
        Tok::Backticked { text, .. } => Some(Field::Attribute(text.clone())),
        _ => None,
    }
}

struct Out {
    suggestions: Vec<Suggestion>,
    lowercase_prefix: String,
    replace: Range<usize>,
}

impl Out {
    fn push(&mut self, text: String, kind: SuggestionKind, detail: Option<String>) {
        if !text.to_lowercase().starts_with(&self.lowercase_prefix)
            || self.suggestions.iter().any(|s| s.text == text)
        {
            return;
        }
        self.suggestions.push(Suggestion {
            text,
            replace: self.replace.clone(),
            kind,
            detail,
        });
    }

    fn push_keyword(&mut self, keyword: &str) {
        self.push(keyword.into(), SuggestionKind::Keyword, None);
    }

    fn push_fields(&mut self, signal: Signal, catalog: &dyn Catalog, include_builtins: bool) {
        if include_builtins {
            for builtin in signal.builtins() {
                self.push(
                    builtin.name().into(),
                    SuggestionKind::Field,
                    Some(format!("built-in, {}", builtin.type_name())),
                );
            }
        }
        for key in catalog.keys(signal, false) {
            let text = Field::Attribute(key.key).to_string();
            self.push(
                text,
                SuggestionKind::Field,
                Some(detail(&key.kind, key.count)),
            );
        }
        // A resource key has no backtick form, so one that needs backticks cannot be written.
        for key in catalog.keys(signal, true) {
            if needs_no_backticks(&key.key) {
                let detail = detail(&key.kind, key.count);
                self.push(
                    Field::Resource(key.key).to_string(),
                    SuggestionKind::Field,
                    Some(detail),
                );
            }
        }
    }

    fn push_operators(&mut self, field: &Field) {
        let (ordered, text) = match field {
            Field::Builtin(builtin) => (builtin.ordered(), builtin.text()),
            _ => (true, true),
        };
        let mut ops = vec![Op::Eq, Op::Ne];
        if ordered {
            ops.extend([Op::Lt, Op::Le, Op::Gt, Op::Ge]);
        }
        for op in ops {
            self.push(op.as_str().into(), SuggestionKind::Operator, None);
        }
        if text {
            self.push("~".into(), SuggestionKind::Operator, None);
        }
        self.push("in".into(), SuggestionKind::Operator, None);
    }

    fn push_values(&mut self, signal: Signal, field: &Field, listed_values: &[ValueInfo]) {
        if let Field::Builtin(builtin) = field {
            for value in builtin.values(signal) {
                self.push((*value).into(), SuggestionKind::Value, None);
            }
            if matches!(builtin, Builtin::Duration) {
                return;
            }
        }
        for info in listed_values {
            let text = value_as_written(&info.value);
            let unquoted_text = match &info.value {
                Value::String(text) => text.to_lowercase(),
                _ => text.to_lowercase(),
            };
            if unquoted_text.starts_with(&self.lowercase_prefix)
                && !self.suggestions.iter().any(|s| s.text == text)
            {
                self.suggestions.push(Suggestion {
                    text,
                    replace: self.replace.clone(),
                    kind: SuggestionKind::Value,
                    detail: Some(info.count.to_string()),
                });
            }
        }
    }
}

fn detail(kind: &str, count: u64) -> String {
    format!("{kind}, {count}")
}
