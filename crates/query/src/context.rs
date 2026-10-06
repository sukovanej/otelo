use std::ops::Range;

use crate::lexer::{Token, TokenType, is_keyword, lex_tokens};
use crate::parser::parse_conjunct_tokens;
use crate::{Expression, Signal};

#[must_use]
pub fn find_context_of_cursor(input: &str, cursor: usize, signal: Signal) -> Option<Expression> {
    let tokens = lex_tokens(input);
    let mut reader = ConjunctReader {
        input,
        tokens: &tokens,
        next_token_index: 0,
        signal,
    };
    let group = reader.read_group(0, false);
    let mut context_terms = Vec::new();
    group.collect_context_of_cursor(cursor, &mut context_terms);
    join_with_and(context_terms)
}

#[must_use]
pub fn join_with_and(mut terms: Vec<Expression>) -> Option<Expression> {
    match terms.len() {
        0 => None,
        1 => terms.pop(),
        _ => Some(Expression::And(terms)),
    }
}

struct Group {
    branches: Vec<Branch>,
}

struct Branch {
    start: usize,
    conjuncts: Vec<Conjunct>,
}

struct Conjunct {
    byte_range: Range<usize>,
    expression: Option<Expression>,
    inner_group: Option<Group>,
}

impl Group {
    fn collect_context_of_cursor(&self, cursor: usize, context_terms: &mut Vec<Expression>) {
        let Some(branch) = self
            .branches
            .iter()
            .rev()
            .find(|branch| branch.start <= cursor)
        else {
            return;
        };
        for conjunct in &branch.conjuncts {
            let holds_cursor =
                conjunct.byte_range.start <= cursor && cursor <= conjunct.byte_range.end;
            if holds_cursor {
                if let Some(inner_group) = &conjunct.inner_group {
                    inner_group.collect_context_of_cursor(cursor, context_terms);
                }
            } else if let Some(expression) = &conjunct.expression {
                context_terms.push(expression.clone());
            }
        }
    }
}

struct ConjunctReader<'a> {
    input: &'a str,
    tokens: &'a [Token],
    next_token_index: usize,
    signal: Signal,
}

impl ConjunctReader<'_> {
    fn peek_token_type(&self) -> Option<&TokenType> {
        self.tokens
            .get(self.next_token_index)
            .map(|token| &token.token_type)
    }

    fn next_is_keyword(&self, keyword: &str) -> bool {
        matches!(self.peek_token_type(), Some(TokenType::Word(word)) if word.eq_ignore_ascii_case(keyword))
    }

    fn skip_token_if(&mut self, wanted: impl Fn(&TokenType) -> bool) -> bool {
        let skipped = self.peek_token_type().is_some_and(wanted);
        if skipped {
            self.next_token_index += 1;
        }
        skipped
    }

    fn read_group(&mut self, start: usize, inside_parentheses: bool) -> Group {
        let mut branches = vec![Branch {
            start,
            conjuncts: Vec::new(),
        }];
        while let Some(token) = self.tokens.get(self.next_token_index) {
            match &token.token_type {
                TokenType::CloseParen if inside_parentheses => break,
                TokenType::Word(word) if word.eq_ignore_ascii_case("or") => {
                    self.next_token_index += 1;
                    branches.push(Branch {
                        start: token.byte_range.end,
                        conjuncts: Vec::new(),
                    });
                }
                TokenType::Word(word) if word.eq_ignore_ascii_case("and") => {
                    self.next_token_index += 1;
                }
                _ => {
                    let conjunct = self.read_conjunct();
                    if let Some(branch) = branches.last_mut() {
                        branch.conjuncts.push(conjunct);
                    }
                }
            }
        }
        Group { branches }
    }

    fn read_conjunct(&mut self) -> Conjunct {
        let first_token_index = self.next_token_index;
        while self.next_is_keyword("not") {
            self.next_token_index += 1;
        }
        let mut reaches_end_of_input = false;
        let inner_group = match self.tokens.get(self.next_token_index) {
            Some(token) if token.token_type == TokenType::OpenParen => {
                self.next_token_index += 1;
                let group = self.read_group(token.byte_range.end, true);
                reaches_end_of_input =
                    !self.skip_token_if(|token_type| *token_type == TokenType::CloseParen);
                Some(group)
            }
            _ => {
                self.skip_term();
                None
            }
        };
        if self.next_token_index == first_token_index {
            self.next_token_index += 1;
        }
        let tokens = &self.tokens[first_token_index..self.next_token_index];
        let start = tokens.first().map_or(0, |token| token.byte_range.start);
        let end = if reaches_end_of_input {
            self.input.len()
        } else {
            tokens.last().map_or(start, |token| token.byte_range.end)
        };
        Conjunct {
            byte_range: start..end,
            expression: parse_conjunct_tokens(self.input, tokens, self.signal),
            inner_group,
        }
    }

    fn skip_term(&mut self) {
        if self.next_is_keyword("has")
            && self
                .tokens
                .get(self.next_token_index + 1)
                .is_some_and(|token| token.token_type == TokenType::OpenParen)
        {
            self.next_token_index += 2;
            self.skip_token_if(is_field_token);
            self.skip_token_if(|token_type| *token_type == TokenType::CloseParen);
            return;
        }
        if !self.skip_token_if(is_field_token) {
            return;
        }
        if self.skip_token_if(|token_type| {
            matches!(token_type, TokenType::Operator(_) | TokenType::Tilde)
        }) {
            self.skip_token_if(is_value_token);
            return;
        }
        if self.next_is_keyword("in") {
            self.next_token_index += 1;
            if self.skip_token_if(|token_type| *token_type == TokenType::OpenParen) {
                while self.skip_token_if(|token_type| {
                    is_value_token(token_type) || *token_type == TokenType::Comma
                }) {}
                self.skip_token_if(|token_type| *token_type == TokenType::CloseParen);
            }
        }
    }
}

fn is_field_token(token_type: &TokenType) -> bool {
    match token_type {
        TokenType::Word(word) => !is_keyword(word),
        TokenType::Backticked { .. } => true,
        _ => false,
    }
}

fn is_value_token(token_type: &TokenType) -> bool {
    match token_type {
        TokenType::Word(word) => !is_keyword(word),
        TokenType::Quoted { .. }
        | TokenType::Int(_)
        | TokenType::Float(_)
        | TokenType::DurationNanos(_) => true,
        _ => false,
    }
}
