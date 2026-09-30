use std::ops::Range;

use crate::lexer::{TokenType, is_keyword, lex_tokens};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightKind {
    Field,
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
    Token(&'a TokenType),
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
        .map(|(index, token)| {
            let (kind, position_after_token) = match &token.token_type {
                TokenType::Word(word) => {
                    let following = match tokens.get(index + 1) {
                        Some(next) => Following::Token(&next.token_type),
                        None if token.byte_range.end < input.len() => Following::Whitespace,
                        None => Following::Nothing,
                    };
                    highlight_word(word, position, following)
                }
                TokenType::Backticked { closed: true, .. } => {
                    (HighlightKind::Field, Position::AfterField)
                }
                TokenType::Quoted { closed: true, .. } => (HighlightKind::String, Position::Term),
                TokenType::Quoted { closed: false, .. }
                | TokenType::Backticked { closed: false, .. }
                | TokenType::Unreadable(_) => (HighlightKind::Invalid, Position::Term),
                TokenType::Int(_) | TokenType::Float(_) | TokenType::DurationNanos(_) => {
                    (HighlightKind::Number, Position::Term)
                }
                TokenType::Operator(_) | TokenType::Tilde => {
                    (HighlightKind::Operator, Position::Value)
                }
                TokenType::Comma => (HighlightKind::Punctuation, Position::Value),
                TokenType::OpenParen if position == Position::AfterIn => {
                    (HighlightKind::Punctuation, Position::Value)
                }
                TokenType::OpenParen if position == Position::AfterHas => {
                    (HighlightKind::Punctuation, Position::HasField)
                }
                TokenType::OpenParen | TokenType::CloseParen => {
                    (HighlightKind::Punctuation, Position::Term)
                }
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
            && matches!(following, Following::Token(TokenType::OpenParen)) =>
        {
            (HighlightKind::Keyword, Position::AfterHas)
        }
        _ if ends_a_field_name(following) => (HighlightKind::Field, Position::AfterField),
        _ => (HighlightKind::UndecidedWord, Position::Term),
    }
}

fn ends_a_field_name(following: Following) -> bool {
    match following {
        Following::Token(TokenType::Operator(_) | TokenType::Tilde) | Following::Whitespace => true,
        Following::Token(TokenType::Word(word)) => word.eq_ignore_ascii_case("in"),
        Following::Token(_) | Following::Nothing => false,
    }
}
