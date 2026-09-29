use std::fmt;

use crate::lexer::{Tok, Token, is_keyword, lex_tokens};
use crate::{Builtin, Expr, Field, Op, Query, Signal, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub position: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "column {}: {}", self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(input: &str, signal: Signal) -> Result<Query, ParseError> {
    let tokens = lex_tokens(input);
    let mut parser = Parser {
        input,
        tokens: &tokens,
        next: 0,
        signal,
    };
    if tokens.is_empty() {
        return Ok(Query::all(signal));
    }
    let expr = parser.or()?;
    if let Some(token) = parser.peek() {
        return Err(parser.error_at(token, "expected AND, OR, or the end of the query"));
    }
    Ok(Query {
        signal,
        expr: Some(expr),
    })
}

struct Parser<'a> {
    input: &'a str,
    tokens: &'a [Token],
    next: usize,
    signal: Signal,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.next)
    }

    fn bump(&mut self) -> Option<&'a Token> {
        let token = self.tokens.get(self.next);
        self.next += 1;
        token
    }

    fn keyword(&self, keyword: &str) -> bool {
        matches!(self.peek(), Some(Token { tok: Tok::Word(word), .. }) if word.eq_ignore_ascii_case(keyword))
    }

    fn error_at(&self, token: &Token, message: &str) -> ParseError {
        let message = match &token.tok {
            Tok::Unreadable(reason) => reason.clone(),
            Tok::Quoted { closed: false, .. } | Tok::Backticked { closed: false, .. } => {
                "the string has no closing quote".into()
            }
            _ => format!(
                "{message}, found {:?}",
                &self.input[token.byte_range.clone()]
            ),
        };
        self.error(token.byte_range.start, message)
    }

    fn error(&self, position: usize, message: String) -> ParseError {
        ParseError {
            position,
            column: self.input[..position].chars().count() + 1,
            message,
        }
    }

    fn error_at_next_token(&self, what: &str) -> ParseError {
        self.peek().map_or_else(
            || {
                self.error(
                    self.input.len(),
                    format!("{what}, found the end of the query"),
                )
            },
            |token| self.error_at(token, what),
        )
    }

    fn or(&mut self) -> Result<Expr, ParseError> {
        let mut terms = vec![self.and()?];
        while self.keyword("or") {
            self.bump();
            terms.push(self.and()?);
        }
        Ok(if terms.len() == 1 {
            terms.remove(0)
        } else {
            Expr::Or(terms)
        })
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        let mut terms = vec![self.not()?];
        loop {
            if self.keyword("and") {
                self.bump();
            } else if !self.next_starts_term() {
                break;
            }
            terms.push(self.not()?);
        }
        Ok(if terms.len() == 1 {
            terms.remove(0)
        } else {
            Expr::And(terms)
        })
    }

    fn next_starts_term(&self) -> bool {
        match self.peek().map(|token| &token.tok) {
            Some(Tok::Word(word)) => !word.eq_ignore_ascii_case("or"),
            Some(Tok::Backticked { .. } | Tok::LParen) => true,
            _ => false,
        }
    }

    fn not(&mut self) -> Result<Expr, ParseError> {
        if self.keyword("not") {
            self.bump();
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let Some(token) = self.peek() else {
            return Err(self.error_at_next_token("expected a field"));
        };
        if token.tok == Tok::LParen {
            self.bump();
            let expr = self.or()?;
            if self.peek().is_some_and(|t| t.tok == Tok::RParen) {
                self.bump();
                return Ok(expr);
            }
            return Err(self.error_at_next_token("expected )"));
        }
        if self.keyword("has")
            && self
                .tokens
                .get(self.next + 1)
                .is_some_and(|t| t.tok == Tok::LParen)
        {
            self.next += 2;
            let field = self.field()?;
            if self.peek().is_some_and(|t| t.tok == Tok::RParen) {
                self.bump();
                return Ok(Expr::Has(field));
            }
            return Err(self.error_at_next_token("expected )"));
        }
        let field = self.field()?;
        let Some(token) = self.bump() else {
            return Err(self.error_at_next_token(&format!("expected an operator after {field}")));
        };
        match &token.tok {
            Tok::Op(op) => Ok(Expr::Compare {
                field,
                op: *op,
                value: self.value()?,
            }),
            Tok::Tilde => match self.value()? {
                Value::String(text) => Ok(Expr::Contains { field, text }),
                _ => Err(self.error(token.byte_range.end, "~ takes a string".into())),
            },
            Tok::Word(word) if word.eq_ignore_ascii_case("in") => {
                if self.bump().is_none_or(|t| t.tok != Tok::LParen) {
                    self.next -= 1;
                    return Err(self.error_at_next_token("expected ( after in"));
                }
                let mut values = vec![self.value()?];
                loop {
                    match self.bump().map(|t| &t.tok) {
                        Some(Tok::Comma) => values.push(self.value()?),
                        Some(Tok::RParen) => break,
                        _ => {
                            self.next -= 1;
                            return Err(self.error_at_next_token("expected , or )"));
                        }
                    }
                }
                Ok(Expr::In { field, values })
            }
            _ => {
                self.next -= 1;
                Err(self.error_at_next_token(&format!(
                    "expected an operator after {field}: =, !=, <, <=, >, >=, ~, or in"
                )))
            }
        }
    }

    fn field(&mut self) -> Result<Field, ParseError> {
        let field = match self.peek().map(|t| &t.tok) {
            Some(Tok::Word(word))
                if !is_keyword(word)
                    && !word.starts_with(|c: char| c.is_ascii_digit() || c == '-') =>
            {
                resolve_field(self.signal, word)
            }
            Some(Tok::Backticked { text, closed: true }) => Field::Attribute(text.clone()),
            _ => return Err(self.error_at_next_token("expected a field")),
        };
        self.bump();
        Ok(field)
    }

    fn value(&mut self) -> Result<Value, ParseError> {
        let value = match self.peek().map(|t| &t.tok) {
            Some(Tok::Quoted { text, closed: true }) => Value::String(text.clone()),
            Some(Tok::Word(word)) if word == "true" => Value::Bool(true),
            Some(Tok::Word(word)) if word == "false" => Value::Bool(false),
            Some(Tok::Word(word)) if !is_keyword(word) => Value::String(word.clone()),
            Some(Tok::Int(n)) => Value::Int(*n),
            Some(Tok::Float(x)) => Value::Float(*x),
            Some(Tok::DurationNanos(ns)) => Value::Duration(*ns),
            _ => return Err(self.error_at_next_token("expected a value")),
        };
        self.bump();
        Ok(value)
    }
}

pub fn resolve_field(signal: Signal, word: &str) -> Field {
    if let Some(key) = word.strip_prefix("resource.") {
        return Field::Resource(key.to_owned());
    }
    if let Some(key) = word.strip_prefix("attr.") {
        return Field::Attribute(key.to_owned());
    }
    Builtin::find(signal, word).map_or_else(|| Field::Attribute(word.to_owned()), Field::Builtin)
}

impl Op {
    #[must_use]
    pub const fn negate(self) -> Self {
        match self {
            Self::Eq => Self::Ne,
            Self::Ne => Self::Eq,
            Self::Lt => Self::Ge,
            Self::Le => Self::Gt,
            Self::Gt => Self::Le,
            Self::Ge => Self::Lt,
        }
    }
}
