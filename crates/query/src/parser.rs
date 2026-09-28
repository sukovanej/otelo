use std::fmt;

use crate::lexer::{Tok, Token, is_keyword, lex};
use crate::{Builtin, Expr, Field, Op, Query, Signal, Value};

/// Why a query does not parse, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// The byte offset in the query.
    pub position: usize,
    /// The 1-based column of the character at `position`.
    pub column: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "column {}: {}", self.column, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parses `input` as a query over `signal`. Whitespace alone is the query
/// that keeps every record.
///
/// # Errors
///
/// When `input` is not a query.
pub fn parse(input: &str, signal: Signal) -> Result<Query, ParseError> {
    let tokens = lex(input);
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
            Tok::Bad(why) => why.clone(),
            Tok::Quoted { closed: false, .. } | Tok::Backticked { closed: false, .. } => {
                "the string has no closing quote".into()
            }
            _ => format!("{message}, found {:?}", &self.input[token.span.clone()]),
        };
        self.error(token.span.start, message)
    }

    fn error(&self, position: usize, message: String) -> ParseError {
        ParseError {
            position,
            column: self.input[..position].chars().count() + 1,
            message,
        }
    }

    /// An error at the next token, or at the end of the input.
    fn expected(&self, what: &str) -> ParseError {
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
            } else if !self.starts_term() {
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

    /// Whether the next token starts a term, so it joins the one before with
    /// `AND`.
    fn starts_term(&self) -> bool {
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
            return Err(self.expected("expected a field"));
        };
        if token.tok == Tok::LParen {
            self.bump();
            let expr = self.or()?;
            if self.peek().is_some_and(|t| t.tok == Tok::RParen) {
                self.bump();
                return Ok(expr);
            }
            return Err(self.expected("expected )"));
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
            return Err(self.expected("expected )"));
        }
        let field = self.field()?;
        let Some(token) = self.bump() else {
            return Err(self.expected(&format!("expected an operator after {field}")));
        };
        match &token.tok {
            Tok::Op(op) => Ok(Expr::Compare {
                field,
                op: *op,
                value: self.value()?,
            }),
            Tok::Tilde => match self.value()? {
                Value::String(text) => Ok(Expr::Contains { field, text }),
                _ => Err(self.error(token.span.end, "~ takes a string".into())),
            },
            Tok::Word(word) if word.eq_ignore_ascii_case("in") => {
                if self.bump().is_none_or(|t| t.tok != Tok::LParen) {
                    self.next -= 1;
                    return Err(self.expected("expected ( after in"));
                }
                let mut values = vec![self.value()?];
                loop {
                    match self.bump().map(|t| &t.tok) {
                        Some(Tok::Comma) => values.push(self.value()?),
                        Some(Tok::RParen) => break,
                        _ => {
                            self.next -= 1;
                            return Err(self.expected("expected , or )"));
                        }
                    }
                }
                Ok(Expr::In { field, values })
            }
            _ => {
                self.next -= 1;
                Err(self.expected(&format!(
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
                resolve(self.signal, word)
            }
            Some(Tok::Backticked { text, closed: true }) => Field::Attribute(text.clone()),
            _ => return Err(self.expected("expected a field")),
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
            Some(Tok::Duration(ns)) => Value::Duration(*ns),
            _ => return Err(self.expected("expected a value")),
        };
        self.bump();
        Ok(value)
    }
}

/// The field a word names in a query over `signal`.
pub fn resolve(signal: Signal, word: &str) -> Field {
    if let Some(key) = word.strip_prefix("resource.") {
        return Field::Resource(key.to_owned());
    }
    if let Some(key) = word.strip_prefix("attr.") {
        return Field::Attribute(key.to_owned());
    }
    Builtin::find(signal, word).map_or_else(|| Field::Attribute(word.to_owned()), Field::Builtin)
}

impl Op {
    /// The operator that holds when this one does not.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(signal: Signal, input: &str) -> String {
        parse(input, signal).unwrap().to_string()
    }

    fn error(input: &str) -> String {
        parse(input, Signal::Logs).unwrap_err().to_string()
    }

    #[test]
    fn parses_the_example() {
        let query = parse(
            r#"http.route = "/matches" OR (user.id = 7 AND http.response.status_code = 200)"#,
            Signal::Spans,
        )
        .unwrap();
        let attr = |key: &str| Field::Attribute(key.into());
        assert_eq!(
            query.expr,
            Some(Expr::Or(vec![
                Expr::Compare {
                    field: attr("http.route"),
                    op: Op::Eq,
                    value: Value::String("/matches".into()),
                },
                Expr::And(vec![
                    Expr::Compare {
                        field: attr("user.id"),
                        op: Op::Eq,
                        value: Value::Int(7),
                    },
                    Expr::Compare {
                        field: attr("http.response.status_code"),
                        op: Op::Eq,
                        value: Value::Int(200),
                    },
                ]),
            ]))
        );
    }

    #[test]
    fn and_binds_tighter_than_or_and_terms_join_with_and() {
        assert_eq!(
            roundtrip(Signal::Logs, "a = 1 or b = 2 c = 3"),
            "a = 1 OR b = 2 AND c = 3"
        );
        assert_eq!(
            roundtrip(Signal::Logs, "(a = 1 or b = 2) and not c = 3"),
            "(a = 1 OR b = 2) AND NOT c = 3"
        );
    }

    #[test]
    fn resolves_builtins_per_signal_and_the_prefixes() {
        let query = parse(
            "name = x resource.host.name = y attr.level = z",
            Signal::Logs,
        )
        .unwrap();
        assert_eq!(
            query.fields(),
            [
                &Field::Attribute("name".into()),
                &Field::Resource("host.name".into()),
                &Field::Attribute("level".into()),
            ]
        );
        let query = parse("name = x `odd key` = 1", Signal::Spans).unwrap();
        assert_eq!(
            query.fields(),
            [
                &Field::Builtin(Builtin::Name),
                &Field::Attribute("odd key".into())
            ]
        );
        assert_eq!(
            roundtrip(Signal::Logs, "attr.level = 1 `a b` = 2 attr.and = 3"),
            "attr.level = 1 AND `a b` = 2 AND attr.and = 3"
        );
    }

    #[test]
    fn parses_every_operator_and_value() {
        assert_eq!(
            roundtrip(
                Signal::Spans,
                r#"duration >= 1.5s status != error error = true x < -2.5 body ~ 'a "b"' has(db.system) k in (1, "two", three)"#,
            ),
            r#"duration >= 1500000000ns AND status != "error" AND error = true AND x < -2.5 AND attr.body ~ "a \"b\"" AND has(db.system) AND k in (1, "two", "three")"#
        );
    }

    #[test]
    fn an_empty_query_keeps_everything() {
        assert_eq!(parse("  ", Signal::Logs).unwrap(), Query::all(Signal::Logs));
    }

    #[test]
    fn says_where_and_what_is_wrong() {
        assert_eq!(
            error("user.id"),
            "column 8: expected an operator after user.id, found the end of the query"
        );
        assert_eq!(
            error("user.id 7"),
            "column 9: expected an operator after user.id: =, !=, <, <=, >, >=, ~, or in, found \"7\""
        );
        assert_eq!(
            error("a = 1 or"),
            "column 9: expected a field, found the end of the query"
        );
        assert_eq!(
            error("(a = 1"),
            "column 7: expected ), found the end of the query"
        );
        assert_eq!(
            error("a = \"open"),
            "column 5: the string has no closing quote"
        );
        assert_eq!(
            error("a = 1 )"),
            "column 7: expected AND, OR, or the end of the query, found \")\""
        );
        assert_eq!(error("a ! 1"), "column 3: unexpected '!'");
        assert_eq!(error("body ~ 5"), "column 7: ~ takes a string");
        assert_eq!(
            error("k in 1"),
            "column 6: expected ( after in, found \"1\""
        );
    }
}
