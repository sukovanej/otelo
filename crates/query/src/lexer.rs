use std::ops::Range;

use crate::Op;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    /// A name, a keyword, or a bare value.
    Word(String),
    Quoted {
        text: String,
        closed: bool,
    },
    Backticked {
        text: String,
        closed: bool,
    },
    Int(i64),
    Float(f64),
    /// In nanoseconds.
    Duration(i64),
    LParen,
    RParen,
    Comma,
    Op(Op),
    Tilde,
    /// Text the lexer cannot read, and why.
    Bad(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    /// Byte offsets in the input.
    pub span: Range<usize>,
}

const UNITS: [(&str, f64); 8] = [
    ("ns", 1.0),
    ("us", 1e3),
    ("µs", 1e3),
    ("ms", 1e6),
    ("s", 1e9),
    ("m", 60e9),
    ("h", 3600e9),
    ("d", 86_400e9),
];

/// A character that can be part of a word.
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || "_.-/:@".contains(c)
}

/// Whether `key` reads back as one word that is not a number.
pub fn is_plain_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '@')
        && chars.all(is_word_char)
}

pub fn is_keyword(word: &str) -> bool {
    ["and", "or", "not"]
        .iter()
        .any(|keyword| word.eq_ignore_ascii_case(keyword))
}

/// Splits `input` into tokens. It never fails: text it cannot read becomes a
/// [`Tok::Bad`], and a string without its closing quote is marked open.
pub fn lex(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        chars.next();
        let next = chars.peek().map(|&(_, c)| c);
        let tok = match c {
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            ',' => Tok::Comma,
            '~' => Tok::Tilde,
            '=' => {
                if next == Some('=') {
                    chars.next();
                }
                Tok::Op(Op::Eq)
            }
            '!' if next == Some('=') => {
                chars.next();
                Tok::Op(Op::Ne)
            }
            '<' | '>' => {
                let or_equal = next == Some('=');
                if or_equal {
                    chars.next();
                }
                Tok::Op(match (c, or_equal) {
                    ('<', false) => Op::Lt,
                    ('<', true) => Op::Le,
                    ('>', false) => Op::Gt,
                    _ => Op::Ge,
                })
            }
            '"' | '\'' | '`' => {
                let (text, closed) = quoted(&mut chars, c);
                if c == '`' {
                    Tok::Backticked { text, closed }
                } else {
                    Tok::Quoted { text, closed }
                }
            }
            c if is_word_char(c) => {
                let mut end = start + c.len_utf8();
                while let Some(&(i, c)) = chars.peek() {
                    if !is_word_char(c) {
                        break;
                    }
                    end = i + c.len_utf8();
                    chars.next();
                }
                word(&input[start..end])
            }
            c => Tok::Bad(format!("unexpected {c:?}")),
        };
        let end = chars.peek().map_or(input.len(), |&(i, _)| i);
        tokens.push(Token {
            tok,
            span: start..end,
        });
    }
    tokens
}

/// The rest of a string that opened with `quote`, and whether it closed.
fn quoted(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    quote: char,
) -> (String, bool) {
    let mut text = String::new();
    while let Some((_, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, 'n')) => text.push('\n'),
                Some((_, 't')) => text.push('\t'),
                Some((_, c)) => text.push(c),
                None => return (text, false),
            },
            c if c == quote => return (text, true),
            c => text.push(c),
        }
    }
    (text, false)
}

/// A number, a duration, or else a word.
fn word(text: &str) -> Tok {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if !digits.starts_with(|c: char| c.is_ascii_digit()) {
        return Tok::Word(text.to_owned());
    }
    if digits.bytes().all(|b| b.is_ascii_digit()) {
        return text
            .parse()
            .map_or_else(|_| Tok::Bad(format!("{text} is too large")), Tok::Int);
    }
    let number_end = digits
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(digits.len());
    let (number, unit) = digits.split_at(number_end);
    let Ok(value) = number.parse::<f64>() else {
        return Tok::Word(text.to_owned());
    };
    if unit.is_empty() {
        return Tok::Float(if text.starts_with('-') { -value } else { value });
    }
    match UNITS.iter().find(|(name, _)| *name == unit) {
        Some((_, factor)) if !text.starts_with('-') => {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "rounded to whole nanoseconds"
            )]
            let ns = (value * factor).round() as i64;
            Tok::Duration(ns)
        }
        _ => Tok::Word(text.to_owned()),
    }
}
