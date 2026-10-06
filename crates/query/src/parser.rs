use std::fmt;

use crate::lexer::{Token, TokenType, is_keyword, lex_tokens};
use crate::{BuiltinField, Expression, Field, Query, Signal, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub byte_offset: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "column {}: {}", self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse_query(input: &str, signal: Signal) -> Result<Query, ParseError> {
    let tokens = lex_tokens(input);
    let mut parser = Parser {
        input,
        tokens: &tokens,
        next_token_index: 0,
        signal,
    };
    if tokens.is_empty() {
        return Ok(Query::all(signal));
    }
    let expression = parser.parse_or()?;
    if let Some(token) = parser.peek_token() {
        return Err(parser.error_at_token(token, "expected AND, OR, or the end of the query"));
    }
    Ok(Query {
        signal,
        expression: Some(expression),
    })
}

pub fn parse_conjunct_tokens(input: &str, tokens: &[Token], signal: Signal) -> Option<Expression> {
    let mut parser = Parser {
        input,
        tokens,
        next_token_index: 0,
        signal,
    };
    let expression = parser.parse_not().ok()?;
    (parser.next_token_index == tokens.len()).then_some(expression)
}

struct Parser<'a> {
    input: &'a str,
    tokens: &'a [Token],
    next_token_index: usize,
    signal: Signal,
}

impl<'a> Parser<'a> {
    fn peek_token(&self) -> Option<&'a Token> {
        self.tokens.get(self.next_token_index)
    }

    fn take_token(&mut self) -> Option<&'a Token> {
        let token = self.tokens.get(self.next_token_index);
        self.next_token_index += 1;
        token
    }

    fn next_is_keyword(&self, keyword: &str) -> bool {
        matches!(self.peek_token(), Some(Token { token_type: TokenType::Word(word), .. }) if word.eq_ignore_ascii_case(keyword))
    }

    fn error_at_token(&self, token: &Token, expectation: &str) -> ParseError {
        let message = match &token.token_type {
            TokenType::Unreadable(reason) => reason.clone(),
            TokenType::Quoted { closed: false, .. }
            | TokenType::Backticked { closed: false, .. } => {
                "the string has no closing quote".into()
            }
            _ => format!(
                "{expectation}, found {:?}",
                &self.input[token.byte_range.clone()]
            ),
        };
        self.error_at_byte(token.byte_range.start, message)
    }

    fn error_at_byte(&self, byte_offset: usize, message: String) -> ParseError {
        ParseError {
            byte_offset,
            column: self.input[..byte_offset].chars().count() + 1,
            message,
        }
    }

    fn error_at_next_token(&self, expectation: &str) -> ParseError {
        self.peek_token().map_or_else(
            || {
                self.error_at_byte(
                    self.input.len(),
                    format!("{expectation}, found the end of the query"),
                )
            },
            |token| self.error_at_token(token, expectation),
        )
    }

    fn parse_or(&mut self) -> Result<Expression, ParseError> {
        let mut terms = vec![self.parse_and()?];
        while self.next_is_keyword("or") {
            self.take_token();
            terms.push(self.parse_and()?);
        }
        Ok(if terms.len() == 1 {
            terms.remove(0)
        } else {
            Expression::Or(terms)
        })
    }

    fn parse_and(&mut self) -> Result<Expression, ParseError> {
        let mut terms = vec![self.parse_not()?];
        loop {
            if self.next_is_keyword("and") {
                self.take_token();
            } else if !self.next_starts_term() {
                break;
            }
            terms.push(self.parse_not()?);
        }
        Ok(if terms.len() == 1 {
            terms.remove(0)
        } else {
            Expression::And(terms)
        })
    }

    fn next_starts_term(&self) -> bool {
        match self.peek_token().map(|token| &token.token_type) {
            Some(TokenType::Word(word)) => !word.eq_ignore_ascii_case("or"),
            Some(TokenType::Backticked { .. } | TokenType::OpenParen) => true,
            _ => false,
        }
    }

    fn parse_not(&mut self) -> Result<Expression, ParseError> {
        if self.next_is_keyword("not") {
            self.take_token();
            return Ok(Expression::Not(Box::new(self.parse_not()?)));
        }
        self.parse_term()
    }

    fn parse_term(&mut self) -> Result<Expression, ParseError> {
        let Some(token) = self.peek_token() else {
            return Err(self.error_at_next_token("expected a field"));
        };
        if token.token_type == TokenType::OpenParen {
            self.take_token();
            let expression = self.parse_or()?;
            if self
                .peek_token()
                .is_some_and(|token| token.token_type == TokenType::CloseParen)
            {
                self.take_token();
                return Ok(expression);
            }
            return Err(self.error_at_next_token("expected )"));
        }
        if self.next_is_keyword("has")
            && self
                .tokens
                .get(self.next_token_index + 1)
                .is_some_and(|token| token.token_type == TokenType::OpenParen)
        {
            self.next_token_index += 2;
            let field = self.parse_field()?;
            if self
                .peek_token()
                .is_some_and(|token| token.token_type == TokenType::CloseParen)
            {
                self.take_token();
                return Ok(Expression::Has(field));
            }
            return Err(self.error_at_next_token("expected )"));
        }
        let field = self.parse_field()?;
        let Some(token) = self.take_token() else {
            return Err(self.error_at_next_token(&format!("expected an operator after {field}")));
        };
        match &token.token_type {
            TokenType::Operator(operator) => Ok(Expression::Compare {
                field,
                operator: *operator,
                value: self.parse_value()?,
            }),
            TokenType::Tilde => match self.parse_value()? {
                Value::String(text) => Ok(Expression::Contains { field, text }),
                _ => Err(self.error_at_byte(token.byte_range.end, "~ takes a string".into())),
            },
            TokenType::Word(word) if word.eq_ignore_ascii_case("in") => {
                if self
                    .take_token()
                    .is_none_or(|token| token.token_type != TokenType::OpenParen)
                {
                    self.next_token_index -= 1;
                    return Err(self.error_at_next_token("expected ( after in"));
                }
                let mut values = vec![self.parse_value()?];
                loop {
                    match self.take_token().map(|token| &token.token_type) {
                        Some(TokenType::Comma) => values.push(self.parse_value()?),
                        Some(TokenType::CloseParen) => break,
                        _ => {
                            self.next_token_index -= 1;
                            return Err(self.error_at_next_token("expected , or )"));
                        }
                    }
                }
                Ok(Expression::In { field, values })
            }
            _ => {
                self.next_token_index -= 1;
                Err(self.error_at_next_token(&format!(
                    "expected an operator after {field}: =, !=, <, <=, >, >=, ~, or in"
                )))
            }
        }
    }

    fn parse_field(&mut self) -> Result<Field, ParseError> {
        let field = match self.peek_token().map(|token| &token.token_type) {
            Some(TokenType::Word(word))
                if !is_keyword(word)
                    && !word.starts_with(|character: char| {
                        character.is_ascii_digit() || character == '-'
                    }) =>
            {
                resolve_field(self.signal, word)
            }
            Some(TokenType::Backticked { text, closed: true }) => Field::Attribute(text.clone()),
            _ => return Err(self.error_at_next_token("expected a field")),
        };
        self.take_token();
        Ok(field)
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        let Some(value) = self
            .peek_token()
            .and_then(|token| value_of_token(&token.token_type))
        else {
            return Err(self.error_at_next_token("expected a value"));
        };
        self.take_token();
        Ok(value)
    }
}

pub fn value_of_token(token_type: &TokenType) -> Option<Value> {
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

pub fn resolve_field(signal: Signal, word: &str) -> Field {
    if let Some(key) = word.strip_prefix("resource.") {
        return Field::Resource(key.to_owned());
    }
    if let Some(key) = word.strip_prefix("attr.") {
        return Field::Attribute(key.to_owned());
    }
    BuiltinField::find_by_name(signal, word)
        .map_or_else(|| Field::Attribute(word.to_owned()), Field::Builtin)
}
