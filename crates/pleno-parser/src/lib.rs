use pleno_ast::{Expr, Literal, Program, Span, Stmt};
use pleno_lexer::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

pub struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            if self.peek_kind() == &TokenKind::Eof {
                break;
            }
            statements.push(self.parse_stmt()?);
        }
        Ok(Program::new(statements))
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek_kind() {
            TokenKind::KeywordLet => self.parse_let_stmt(),
            _ => {
                let expr = self.parse_expr()?;
                self.expect(TokenKind::Semicolon, "Expected ';' after expression")?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let mutable = if self.matches(&TokenKind::KeywordMut) {
            true
        } else {
            false
        };
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected identifier after 'let', found {other:?}"),
                    span: self.previous_span(),
                })
            }
        };
        let ty = if self.matches(&TokenKind::Colon) {
            match self.advance().kind.clone() {
                TokenKind::Ident(value) => Some(value),
                other => {
                    return Err(ParseError {
                        message: format!("Expected type name after ':', found {other:?}"),
                        span: self.previous_span(),
                    })
                }
            }
        } else {
            None
        };
        let value = if self.matches(&TokenKind::Equal) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after let statement")?;
        Ok(Stmt::Let {
            name,
            mutable,
            ty,
            value,
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance();
        let expr = match token.kind.clone() {
            TokenKind::Int(value) => Expr::Literal(Literal::Int(value)),
            TokenKind::Float(value) => Expr::Literal(Literal::Float(value)),
            TokenKind::String(value) => Expr::Literal(Literal::String(value)),
            TokenKind::Bool(value) => Expr::Literal(Literal::Bool(value)),
            TokenKind::Null => Expr::Literal(Literal::Null),
            TokenKind::Ident(name) => Expr::Ident(name),
            TokenKind::LParen => {
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen, "Expected ')' after expression")?;
                expr
            }
            other => {
                return Err(ParseError {
                    message: format!("Unexpected token in expression: {other:?}"),
                    span: token.span,
                })
            }
        };
        Ok(expr)
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.peek_kind() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<(), ParseError> {
        if self.peek_kind() == &kind {
            self.advance();
            Ok(())
        } else {
            Err(ParseError {
                message: message.to_string(),
                span: self.peek().span,
            })
        }
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("tokens are not empty"))
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.index += 1;
        }
        self.previous()
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.index.saturating_sub(1)]
    }

    fn previous_span(&self) -> Span {
        self.previous().span
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }
}
