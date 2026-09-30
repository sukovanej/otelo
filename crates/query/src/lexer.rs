use std::ops::Range;

use crate::Operator;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenType {
    Word(String),
    Quoted { text: String, closed: bool },
    Backticked { text: String, closed: bool },
    Int(i64),
    Float(f64),
    DurationNanos(i64),
    OpenParen,
    CloseParen,
    Comma,
    Operator(Operator),
    Tilde,
    Unreadable(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    pub byte_range: Range<usize>,
}

const NANOS_PER_DURATION_UNIT: [(&str, f64); 8] = [
    ("ns", 1.0),
    ("us", 1e3),
    ("µs", 1e3),
    ("ms", 1e6),
    ("s", 1e9),
    ("m", 60e9),
    ("h", 3600e9),
    ("d", 86_400e9),
];

pub fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || "_.-/:@".contains(character)
}

pub fn needs_no_backticks(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_' || first == '@')
        && chars.all(is_word_char)
}

pub fn is_keyword(word: &str) -> bool {
    ["and", "or", "not"]
        .iter()
        .any(|keyword| word.eq_ignore_ascii_case(keyword))
}

pub fn lex_tokens(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some(&(start, character)) = chars.peek() {
        if character.is_whitespace() {
            chars.next();
            continue;
        }
        chars.next();
        let next_character = chars.peek().map(|&(_, character)| character);
        let token_type = match character {
            '(' => TokenType::OpenParen,
            ')' => TokenType::CloseParen,
            ',' => TokenType::Comma,
            '~' => TokenType::Tilde,
            '=' => {
                if next_character == Some('=') {
                    chars.next();
                }
                TokenType::Operator(Operator::Eq)
            }
            '!' if next_character == Some('=') => {
                chars.next();
                TokenType::Operator(Operator::Ne)
            }
            '<' | '>' => {
                let or_equal = next_character == Some('=');
                if or_equal {
                    chars.next();
                }
                TokenType::Operator(match (character, or_equal) {
                    ('<', false) => Operator::Lt,
                    ('<', true) => Operator::Le,
                    ('>', false) => Operator::Gt,
                    _ => Operator::Ge,
                })
            }
            '"' | '\'' | '`' => {
                let (text, closed) = read_quoted_text(&mut chars, character);
                if character == '`' {
                    TokenType::Backticked { text, closed }
                } else {
                    TokenType::Quoted { text, closed }
                }
            }
            _ if is_word_char(character) => {
                let mut end = start + character.len_utf8();
                while let Some(&(byte_offset, character)) = chars.peek() {
                    if !is_word_char(character) {
                        break;
                    }
                    end = byte_offset + character.len_utf8();
                    chars.next();
                }
                classify_word(&input[start..end])
            }
            _ => TokenType::Unreadable(format!("unexpected {character:?}")),
        };
        let end = chars
            .peek()
            .map_or(input.len(), |&(byte_offset, _)| byte_offset);
        tokens.push(Token {
            token_type,
            byte_range: start..end,
        });
    }
    tokens
}

fn read_quoted_text(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    closing_quote: char,
) -> (String, bool) {
    let mut text = String::new();
    while let Some((_, character)) = chars.next() {
        match character {
            '\\' => match chars.next() {
                Some((_, 'n')) => text.push('\n'),
                Some((_, 't')) => text.push('\t'),
                Some((_, escaped)) => text.push(escaped),
                None => return (text, false),
            },
            _ if character == closing_quote => return (text, true),
            _ => text.push(character),
        }
    }
    (text, false)
}

fn classify_word(text: &str) -> TokenType {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if !digits.starts_with(|character: char| character.is_ascii_digit()) {
        return TokenType::Word(text.to_owned());
    }
    if digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return text.parse().map_or_else(
            |_| TokenType::Unreadable(format!("{text} is too large")),
            TokenType::Int,
        );
    }
    let number_end = digits
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(digits.len());
    let (number, unit) = digits.split_at(number_end);
    let Ok(value) = number.parse::<f64>() else {
        return TokenType::Word(text.to_owned());
    };
    if unit.is_empty() {
        return TokenType::Float(if text.starts_with('-') { -value } else { value });
    }
    match NANOS_PER_DURATION_UNIT
        .iter()
        .find(|(unit_name, _)| *unit_name == unit)
    {
        Some((_, nanos_per_unit)) if !text.starts_with('-') => {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "rounded to whole nanoseconds"
            )]
            let nanos = (value * nanos_per_unit).round() as i64;
            TokenType::DurationNanos(nanos)
        }
        _ => TokenType::Word(text.to_owned()),
    }
}
