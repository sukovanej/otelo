use std::ops::Range;

use crate::lexer::{Tok, is_keyword, lex_tokens};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightKind {
    Field,
    // A word still being typed, which may become a field or a keyword, such as the `an` of `and`.
    UndecidedWord,
    Operator,
    Keyword,
    String,
    Number,
    Boolean,
    Punctuation,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Highlight {
    pub byte_range: Range<usize>,
    pub kind: HighlightKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    Term,
    AfterField,
    AfterIn,
    AfterHas,
    HasField,
    Value,
}

#[derive(Clone, Copy)]
enum Following<'a> {
    Token(&'a Tok),
    Whitespace,
    Nothing,
}

// The web UI colors a query as it is typed with a port of this function,
// packages/app/src/highlight.ts, which a test holds to the snapshots of this one.
#[must_use]
pub fn highlight_tokens(input: &str) -> Vec<Highlight> {
    let tokens = lex_tokens(input);
    let mut position = Position::Term;
    tokens
        .iter()
        .enumerate()
        .map(|(i, token)| {
            let (kind, position_after_token) = match &token.tok {
                Tok::Word(word) => {
                    let following = match tokens.get(i + 1) {
                        Some(next) => Following::Token(&next.tok),
                        None if token.byte_range.end < input.len() => Following::Whitespace,
                        None => Following::Nothing,
                    };
                    highlight_word(word, position, following)
                }
                Tok::Backticked { closed: true, .. } => {
                    (HighlightKind::Field, Position::AfterField)
                }
                Tok::Quoted { closed: true, .. } => (HighlightKind::String, Position::Term),
                Tok::Quoted { closed: false, .. }
                | Tok::Backticked { closed: false, .. }
                | Tok::Unreadable(_) => (HighlightKind::Invalid, Position::Term),
                Tok::Int(_) | Tok::Float(_) | Tok::DurationNanos(_) => {
                    (HighlightKind::Number, Position::Term)
                }
                Tok::Op(_) | Tok::Tilde => (HighlightKind::Operator, Position::Value),
                Tok::Comma => (HighlightKind::Punctuation, Position::Value),
                Tok::LParen if position == Position::AfterIn => {
                    (HighlightKind::Punctuation, Position::Value)
                }
                Tok::LParen if position == Position::AfterHas => {
                    (HighlightKind::Punctuation, Position::HasField)
                }
                Tok::LParen | Tok::RParen => (HighlightKind::Punctuation, Position::Term),
            };
            position = position_after_token;
            Highlight {
                byte_range: token.byte_range.clone(),
                kind,
            }
        })
        .collect()
}

fn highlight_word(
    word: &str,
    position: Position,
    following: Following,
) -> (HighlightKind, Position) {
    if is_keyword(word) {
        return (HighlightKind::Keyword, Position::Term);
    }
    match position {
        Position::Value if word == "true" || word == "false" => {
            (HighlightKind::Boolean, Position::Term)
        }
        Position::Value => (HighlightKind::String, Position::Term),
        Position::AfterField if word.eq_ignore_ascii_case("in") => {
            (HighlightKind::Keyword, Position::AfterIn)
        }
        Position::HasField => (HighlightKind::Field, Position::AfterField),
        _ if word.eq_ignore_ascii_case("has")
            && matches!(following, Following::Token(Tok::LParen)) =>
        {
            (HighlightKind::Keyword, Position::AfterHas)
        }
        _ if ends_a_field_name(following) => (HighlightKind::Field, Position::AfterField),
        _ => (HighlightKind::UndecidedWord, Position::Term),
    }
}

fn ends_a_field_name(following: Following) -> bool {
    match following {
        Following::Token(Tok::Op(_) | Tok::Tilde) | Following::Whitespace => true,
        Following::Token(Tok::Word(word)) => word.eq_ignore_ascii_case("in"),
        Following::Token(_) | Following::Nothing => false,
    }
}
