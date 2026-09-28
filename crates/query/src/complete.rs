use std::ops::Range;

use crate::lexer::{Tok, Token, is_keyword, is_plain_key, lex};
use crate::parser::resolve;
use crate::{Builtin, Field, Op, Signal, Value, quote};

/// The most suggestions [`complete`] returns.
const MAX_SUGGESTIONS: usize = 50;

/// The attributes and values the records of a signal have, for completion.
pub trait Catalog {
    /// The attribute keys of the records (`resource` false) or of their
    /// resources (`resource` true), the most common first.
    fn keys(&self, signal: Signal, resource: bool) -> Vec<KeyInfo>;

    /// The values `field` has, the most common first.
    fn values(&self, signal: Signal, field: &Field) -> Vec<ValueInfo>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyInfo {
    pub key: String,
    /// The JSON type of the values: `string`, `int`, `float`, `bool`,
    /// `array`, `object`, or `mixed`.
    pub kind: String,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValueInfo {
    pub value: Value,
    pub count: u64,
}

/// A catalog without attributes, so completion offers the built-in fields,
/// the operators, and the keywords.
pub struct NoCatalog;

impl Catalog for NoCatalog {
    fn keys(&self, _: Signal, _: bool) -> Vec<KeyInfo> {
        Vec::new()
    }

    fn values(&self, _: Signal, _: &Field) -> Vec<ValueInfo> {
        Vec::new()
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
    /// The text that replaces `replace`.
    pub text: String,
    /// The byte range of the input that the text replaces: the word at the
    /// cursor, or an empty range at the cursor.
    pub replace: Range<usize>,
    pub kind: SuggestionKind,
    /// The type of a field, or how many records have a field or a value.
    pub detail: Option<String>,
}

/// What the grammar expects where the cursor is.
#[derive(Clone, Debug)]
enum Expect {
    /// A field, `NOT`, `(`, or `has(`.
    Term,
    Operator(Field),
    Value(Field),
    InOpen,
    InValue(Field),
    InNext,
    HasOpen,
    HasField,
    HasClose,
    /// `AND`, `OR`, `)`, or the next term.
    After,
}

/// Suggests what can go at byte offset `cursor` of a query over `signal`.
#[must_use]
pub fn complete(
    input: &str,
    cursor: usize,
    signal: Signal,
    catalog: &dyn Catalog,
) -> Vec<Suggestion> {
    let cursor = floor_char_boundary(input, cursor);
    let tokens = lex(&input[..cursor]);
    // The word the cursor touches is what the suggestions complete.
    let (before, partial) = match tokens.last() {
        Some(last) if last.span.end == cursor && completable(&last.tok) => {
            (&tokens[..tokens.len() - 1], Some(last))
        }
        _ => (&tokens[..], None),
    };
    let Some((expect, depth)) = state(before, signal) else {
        return Vec::new();
    };
    let replace = partial.map_or(cursor..cursor, |token| {
        // Replace the whole word, also the part after the cursor.
        let end = input[cursor..]
            .find(|c: char| !crate::lexer::is_word_char(c))
            .map_or(input.len(), |i| cursor + i);
        token.span.start..end.max(cursor)
    });
    let prefix = partial.map_or(String::new(), |token| match &token.tok {
        Tok::Quoted { text, .. } | Tok::Backticked { text, .. } => text.clone(),
        _ => input[token.span.clone()].to_owned(),
    });
    let mut out = Out {
        suggestions: Vec::new(),
        prefix: prefix.to_lowercase(),
        replace,
    };
    match expect {
        Expect::Term => {
            out.fields(signal, catalog, true);
            out.keyword("not");
            out.keyword("has(");
            out.keyword("(");
        }
        Expect::Operator(field) => out.operators(&field),
        Expect::Value(field) | Expect::InValue(field) => out.values(signal, &field, catalog),
        Expect::InOpen | Expect::HasOpen => out.keyword("("),
        Expect::InNext => {
            out.keyword(",");
            out.keyword(")");
        }
        Expect::HasField => out.fields(signal, catalog, false),
        Expect::HasClose => out.keyword(")"),
        Expect::After => {
            out.keyword("and");
            out.keyword("or");
            if depth > 0 {
                out.keyword(")");
            }
            if !out.prefix.is_empty() {
                out.fields(signal, catalog, true);
            }
        }
    }
    out.suggestions.truncate(MAX_SUGGESTIONS);
    out.suggestions
}

/// Whether the cursor at the end of the token is still inside it.
const fn completable(tok: &Tok) -> bool {
    matches!(
        tok,
        Tok::Word(_)
            | Tok::Int(_)
            | Tok::Float(_)
            | Tok::Duration(_)
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

/// What comes after `tokens`, and how many parentheses are open. `None` when
/// the tokens are not the start of a query.
fn state(tokens: &[Token], signal: Signal) -> Option<(Expect, usize)> {
    let mut expect = Expect::Term;
    let mut depth = 0_usize;
    let mut i = 0;
    while i < tokens.len() {
        let tok = &tokens[i].tok;
        let word = match tok {
            Tok::Word(word) => Some(word.to_ascii_lowercase()),
            _ => None,
        };
        expect = match (expect, tok) {
            (Expect::After, Tok::Word(_)) if matches!(word.as_deref(), Some("and" | "or")) => {
                Expect::Term
            }
            (Expect::After, Tok::RParen) if depth > 0 => {
                depth -= 1;
                Expect::After
            }
            // After a term, the next one joins with AND.
            (Expect::Term, _)
            | (Expect::After, Tok::Word(_) | Tok::Backticked { .. } | Tok::LParen) => {
                continue_term(tok, word.as_deref(), tokens.get(i + 1), signal, &mut depth)?
            }
            (Expect::Operator(field), Tok::Op(_) | Tok::Tilde) => Expect::Value(field),
            (Expect::Operator(field), Tok::Word(_)) if word.as_deref() == Some("in") => {
                i += 1;
                match tokens.get(i).map(|t| &t.tok) {
                    Some(Tok::LParen) => Expect::InValue(field),
                    None => Expect::InOpen,
                    Some(_) => return None,
                }
            }
            (Expect::Value(_), tok) if is_value(tok) => Expect::After,
            (Expect::InValue(_), tok) if is_value(tok) => Expect::InNext,
            (Expect::InNext, Tok::Comma) => {
                // The field of the list is the one before `in`.
                let field = in_field(&tokens[..i], signal)?;
                Expect::InValue(field)
            }
            (Expect::HasOpen, Tok::LParen) => Expect::HasField,
            (Expect::HasField, Tok::Word(_) | Tok::Backticked { closed: true, .. }) => {
                Expect::HasClose
            }
            (Expect::InNext | Expect::HasClose, Tok::RParen) => Expect::After,
            _ => return None,
        };
        i += 1;
    }
    Some((expect, depth))
}

/// The state after the first token of a term.
fn continue_term(
    tok: &Tok,
    word: Option<&str>,
    next: Option<&Token>,
    signal: Signal,
    depth: &mut usize,
) -> Option<Expect> {
    Some(match (tok, word) {
        (Tok::LParen, _) => {
            *depth += 1;
            Expect::Term
        }
        (_, Some("not")) => Expect::Term,
        (_, Some("has")) if next.is_none_or(|t| t.tok == Tok::LParen) => Expect::HasOpen,
        (Tok::Word(text), _) if !is_keyword(text) => Expect::Operator(resolve(signal, text)),
        (Tok::Backticked { text, closed: true }, _) => {
            Expect::Operator(Field::Attribute(text.clone()))
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
            | Tok::Duration(_)
            | Tok::Quoted { closed: true, .. }
    )
}

/// The field of the `in` list that `tokens` end inside.
fn in_field(tokens: &[Token], signal: Signal) -> Option<Field> {
    let at = tokens
        .iter()
        .rposition(|t| matches!(&t.tok, Tok::Word(word) if word.eq_ignore_ascii_case("in")))?;
    match &tokens.get(at.checked_sub(1)?)?.tok {
        Tok::Word(word) => Some(resolve(signal, word)),
        Tok::Backticked { text, .. } => Some(Field::Attribute(text.clone())),
        _ => None,
    }
}

struct Out {
    suggestions: Vec<Suggestion>,
    /// The lowercase text of the word at the cursor.
    prefix: String,
    replace: Range<usize>,
}

impl Out {
    fn push(&mut self, text: String, kind: SuggestionKind, detail: Option<String>) {
        if !text.to_lowercase().starts_with(&self.prefix)
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

    fn keyword(&mut self, keyword: &str) {
        self.push(keyword.into(), SuggestionKind::Keyword, None);
    }

    /// The built-in fields, the record attributes, and, when `resources`,
    /// the resource attributes.
    fn fields(&mut self, signal: Signal, catalog: &dyn Catalog, resources: bool) {
        for builtin in signal.builtins() {
            self.push(
                builtin.name().into(),
                SuggestionKind::Field,
                Some("built-in".into()),
            );
        }
        for key in catalog.keys(signal, false) {
            let text = Field::Attribute(key.key).to_string();
            self.push(
                text,
                SuggestionKind::Field,
                Some(detail(&key.kind, key.count)),
            );
        }
        if resources {
            // A resource key has no backtick form.
            for key in catalog.keys(signal, true) {
                if is_plain_key(&key.key) {
                    let detail = detail(&key.kind, key.count);
                    self.push(
                        Field::Resource(key.key).to_string(),
                        SuggestionKind::Field,
                        Some(detail),
                    );
                }
            }
        }
    }

    fn operators(&mut self, field: &Field) {
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

    fn values(&mut self, signal: Signal, field: &Field, catalog: &dyn Catalog) {
        if let Field::Builtin(builtin) = field {
            for value in builtin.values(signal) {
                self.push((*value).into(), SuggestionKind::Value, None);
            }
            if matches!(builtin, Builtin::Duration) {
                return;
            }
        }
        for info in catalog.values(signal, field) {
            let text = match &info.value {
                Value::String(text) => quote(text),
                value => value.to_string(),
            };
            // The prefix is typed without the opening quote.
            let key = match &info.value {
                Value::String(text) => text.to_lowercase(),
                _ => text.to_lowercase(),
            };
            if key.starts_with(&self.prefix) && !self.suggestions.iter().any(|s| s.text == text) {
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;

    impl Catalog for Fake {
        fn keys(&self, signal: Signal, resource: bool) -> Vec<KeyInfo> {
            let key = |key: &str, kind: &str, count| KeyInfo {
                key: key.into(),
                kind: kind.into(),
                count,
            };
            match (signal, resource) {
                (Signal::Spans, false) => vec![
                    key("http.route", "string", 90),
                    key("http.response.status_code", "int", 80),
                    key("user.id", "int", 40),
                    key("level", "string", 1),
                ],
                (_, true) => vec![key("host.name", "string", 3)],
                _ => Vec::new(),
            }
        }

        fn values(&self, _: Signal, field: &Field) -> Vec<ValueInfo> {
            match field {
                Field::Attribute(key) if key == "http.route" => vec![
                    ValueInfo {
                        value: Value::String("/matches".into()),
                        count: 50,
                    },
                    ValueInfo {
                        value: Value::String("/languages".into()),
                        count: 40,
                    },
                ],
                Field::Attribute(key) if key == "http.response.status_code" => vec![ValueInfo {
                    value: Value::Int(200),
                    count: 70,
                }],
                Field::Builtin(Builtin::Service) => vec![ValueInfo {
                    value: Value::String("mudro".into()),
                    count: 9,
                }],
                _ => Vec::new(),
            }
        }
    }

    /// The suggestions at the `|` in `input`.
    fn at(input: &str) -> Vec<String> {
        let cursor = input.find('|').unwrap();
        let input = input.replace('|', "");
        complete(&input, cursor, Signal::Spans, &Fake)
            .into_iter()
            .map(|s| s.text)
            .collect()
    }

    #[test]
    fn suggests_fields_at_the_start_and_by_prefix() {
        let all = at("|");
        assert!(
            all.starts_with(&["service".into(), "name".into()]),
            "{all:?}"
        );
        assert!(all.contains(&"http.route".into()));
        assert!(all.contains(&"attr.level".into()));
        assert!(all.contains(&"resource.host.name".into()));
        assert!(all.contains(&"not".into()));
        assert_eq!(at("http.r|"), ["http.route", "http.response.status_code"]);
        assert_eq!(at("resource.h|"), ["resource.host.name"]);
        assert_eq!(at("a = 1 AND (us|"), ["user.id"]);
    }

    #[test]
    fn suggests_operators_by_the_type_of_the_field() {
        assert_eq!(
            at("http.route |"),
            ["=", "!=", "<", "<=", ">", ">=", "~", "in"]
        );
        assert_eq!(at("error |"), ["=", "!=", "in"]);
        assert_eq!(at("duration |"), ["=", "!=", "<", "<=", ">", ">=", "in"]);
    }

    #[test]
    fn suggests_values_of_the_field() {
        assert_eq!(at("http.route = |"), [r#""/matches""#, r#""/languages""#]);
        assert_eq!(at(r#"http.route = "/l|"#), [r#""/languages""#]);
        assert_eq!(at("http.route = /m|"), [r#""/matches""#]);
        assert_eq!(at("http.response.status_code >= 2|"), ["200"]);
        assert_eq!(at("service = |"), [r#""mudro""#]);
        assert_eq!(at("kind = s|"), ["server"]);
        assert_eq!(at("error = |"), ["true", "false"]);
        assert_eq!(
            at("http.route in (/matches, |"),
            [r#""/matches""#, r#""/languages""#]
        );
    }

    #[test]
    fn suggests_keywords_after_a_term() {
        assert_eq!(at("user.id = 7 |"), ["and", "or"]);
        assert_eq!(at("(user.id = 7 |"), ["and", "or", ")"]);
        assert_eq!(at("user.id = 7 o|"), ["or"]);
        assert_eq!(at("user.id = 7 us|"), ["user.id"]);
        let has = at("has(|");
        assert!(has.contains(&"user.id".into()), "{has:?}");
        assert!(!has.contains(&"resource.host.name".into()), "{has:?}");
        assert_eq!(at("has(user.id |"), [")"]);
    }

    #[test]
    fn replaces_the_whole_word_at_the_cursor() {
        let input = "http.ro = 1";
        let suggestions = complete(input, 4, Signal::Spans, &Fake);
        assert_eq!(suggestions[0].text, "http.route");
        assert_eq!(suggestions[0].replace, 0..7);
        let suggestions = complete("user.id = 7 ", 12, Signal::Spans, &Fake);
        assert_eq!(suggestions[0].replace, 12..12);
    }

    #[test]
    fn suggests_nothing_after_a_broken_start() {
        assert!(at(") |").is_empty());
        assert!(at("a = = |").is_empty());
    }
}
