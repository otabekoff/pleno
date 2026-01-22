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
            TokenKind::KeywordConst => self.parse_const_stmt(),
            TokenKind::KeywordIf => self.parse_if_stmt(),
            TokenKind::KeywordWhile => self.parse_while_stmt(),
            TokenKind::KeywordFor => self.parse_for_stmt(),
            TokenKind::KeywordFn => self.parse_fn_stmt(),
            TokenKind::KeywordReturn => self.parse_return_stmt(),
            TokenKind::KeywordBreak => self.parse_break_stmt(),
            TokenKind::KeywordContinue => self.parse_continue_stmt(),
            TokenKind::LBrace => self.parse_block_stmt(),
            _ => {
                if self.is_assignment() {
                    return self.parse_assign_stmt();
                }
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
            is_const: false,
        })
    }

    fn parse_const_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        if self.matches(&TokenKind::KeywordMut) {
            return Err(ParseError {
                message: "const cannot be mutable".to_string(),
                span: self.previous_span(),
            });
        }
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected identifier after 'const', found {other:?}"),
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
        self.expect(TokenKind::Semicolon, "Expected ';' after const statement")?;
        Ok(Stmt::Let {
            name,
            mutable: false,
            ty,
            value,
            is_const: true,
        })
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        self.expect(TokenKind::LParen, "Expected '(' after 'if'")?;
        let condition = self.parse_expr()?;
        self.expect(TokenKind::RParen, "Expected ')' after if condition")?;
        let then_branch = Box::new(self.parse_stmt()?);
        let else_branch = if self.matches(&TokenKind::KeywordElse) {
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        self.expect(TokenKind::LParen, "Expected '(' after 'while'")?;
        let condition = self.parse_expr()?;
        self.expect(TokenKind::RParen, "Expected ')' after while condition")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While { condition, body })
    }

    fn parse_fn_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(name) => name,
            other => {
                return Err(ParseError {
                    message: format!("Expected function name, found {other:?}"),
                    span: self.previous_span(),
                })
            }
        };
        self.expect(TokenKind::LParen, "Expected '(' after function name")?;
        let mut params = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RParen) {
            loop {
                match self.advance().kind.clone() {
                    TokenKind::Ident(name) => params.push(name),
                    other => {
                        return Err(ParseError {
                            message: format!("Expected parameter name, found {other:?}"),
                            span: self.previous_span(),
                        })
                    }
                }
                if self.matches(&TokenKind::Comma) {
                    if matches!(self.peek_kind(), TokenKind::RParen) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        self.expect(TokenKind::RParen, "Expected ')' after parameters")?;
        let body = Box::new(self.parse_block_stmt()?);
        Ok(Stmt::Fn { name, params, body })
    }

    fn parse_return_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let expr = if matches!(self.peek_kind(), TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after return")?;
        Ok(Stmt::Return(expr))
    }

    fn parse_break_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        self.expect(TokenKind::Semicolon, "Expected ';' after break")?;
        Ok(Stmt::Break)
    }

    fn parse_continue_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        self.expect(TokenKind::Semicolon, "Expected ';' after continue")?;
        Ok(Stmt::Continue)
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        self.expect(TokenKind::LParen, "Expected '(' after 'for'")?;
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(name) => name,
            other => {
                return Err(ParseError {
                    message: format!("Expected identifier in for loop, found {other:?}"),
                    span: self.previous_span(),
                })
            }
        };
        self.expect(TokenKind::KeywordIn, "Expected 'in' in for loop")?;
        let iterable = self.parse_expr()?;
        self.expect(TokenKind::RParen, "Expected ')' after for loop")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For { name, iterable, body })
    }

    fn parse_block_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LBrace, "Expected '{' to start block")?;
        let mut statements = Vec::new();
        while !self.is_at_end() && self.peek_kind() != &TokenKind::RBrace {
            statements.push(self.parse_stmt()?);
        }
        self.expect(TokenKind::RBrace, "Expected '}' after block")?;
        Ok(Stmt::Block(statements))
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_and()?;
        while self.matches(&TokenKind::OrOr) {
            let right = self.parse_and()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: pleno_ast::BinaryOp::Or,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_equality()?;
        while self.matches(&TokenKind::AndAnd) {
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: pleno_ast::BinaryOp::And,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_comparison()?;
        loop {
            let op = if self.matches(&TokenKind::EqualEqual) {
                Some(pleno_ast::BinaryOp::Equal)
            } else if self.matches(&TokenKind::BangEqual) {
                Some(pleno_ast::BinaryOp::NotEqual)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_range()?;
        loop {
            let op = if self.matches(&TokenKind::Less) {
                Some(pleno_ast::BinaryOp::Less)
            } else if self.matches(&TokenKind::LessEqual) {
                Some(pleno_ast::BinaryOp::LessEqual)
            } else if self.matches(&TokenKind::Greater) {
                Some(pleno_ast::BinaryOp::Greater)
            } else if self.matches(&TokenKind::GreaterEqual) {
                Some(pleno_ast::BinaryOp::GreaterEqual)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_range()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_range(&mut self) -> Result<Expr, ParseError> {
        if self.matches(&TokenKind::DotDot) {
            let right = if self.is_range_terminator() {
                None
            } else {
                Some(Box::new(self.parse_term()?))
            };
            return Ok(Expr::Range {
                start: None,
                end: right,
                inclusive: false,
            });
        }
        if self.matches(&TokenKind::DotDotEqual) {
            let right = if self.is_range_terminator() {
                None
            } else {
                Some(Box::new(self.parse_term()?))
            };
            return Ok(Expr::Range {
                start: None,
                end: right,
                inclusive: true,
            });
        }
        let left = self.parse_term()?;
        if self.matches(&TokenKind::DotDot) {
            let right = if self.is_range_terminator() {
                None
            } else {
                Some(Box::new(self.parse_term()?))
            };
            return Ok(Expr::Range {
                start: Some(Box::new(left)),
                end: right,
                inclusive: false,
            });
        }
        if self.matches(&TokenKind::DotDotEqual) {
            let right = if self.is_range_terminator() {
                None
            } else {
                Some(Box::new(self.parse_term()?))
            };
            return Ok(Expr::Range {
                start: Some(Box::new(left)),
                end: right,
                inclusive: true,
            });
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_factor()?;
        loop {
            let op = if self.matches(&TokenKind::Plus) {
                Some(pleno_ast::BinaryOp::Add)
            } else if self.matches(&TokenKind::Minus) {
                Some(pleno_ast::BinaryOp::Subtract)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_factor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.matches(&TokenKind::Star) {
                Some(pleno_ast::BinaryOp::Multiply)
            } else if self.matches(&TokenKind::Slash) {
                Some(pleno_ast::BinaryOp::Divide)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.matches(&TokenKind::Bang) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: pleno_ast::UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        if self.matches(&TokenKind::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: pleno_ast::UnaryOp::Negate,
                expr: Box::new(expr),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance();
        let mut expr = match token.kind.clone() {
            TokenKind::Int(value) => Expr::Literal(Literal::Int(value)),
            TokenKind::Float(value) => Expr::Literal(Literal::Float(value)),
            TokenKind::String(value) => Expr::Literal(Literal::String(value)),
            TokenKind::Bool(value) => Expr::Literal(Literal::Bool(value)),
            TokenKind::Null => Expr::Literal(Literal::Null),
            TokenKind::Ident(name) => Expr::Ident(name),
            TokenKind::LBrace => {
                if matches!(self.peek_kind(), TokenKind::RBrace) {
                    self.expect(TokenKind::RBrace, "Expected '}' after map literal")?;
                    Expr::MapLiteral(Vec::new())
                } else if self.lookahead_is_set_literal()? {
                    self.parse_set_literal()?
                } else {
                    self.parse_map_literal()?
                }
            }
            TokenKind::LBracket => {
                let mut items = Vec::new();
                if !matches!(self.peek_kind(), TokenKind::RBracket) {
                    loop {
                        let value = self.parse_expr()?;
                        items.push(value);
                        if self.matches(&TokenKind::Comma) {
                            if matches!(self.peek_kind(), TokenKind::RBracket) {
                                break;
                            }
                            continue;
                        }
                        break;
                    }
                }
                self.expect(TokenKind::RBracket, "Expected ']' after array literal")?;
                Expr::ArrayLiteral(items)
            }
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
        loop {
            if self.matches(&TokenKind::Dot) {
                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(name) => name,
                    other => {
                        return Err(ParseError {
                            message: format!("Expected identifier after '.', found {other:?}"),
                            span: self.previous_span(),
                        })
                    }
                };
                expr = Expr::Property {
                    object: Box::new(expr),
                    name,
                };
                continue;
            }
            if self.matches(&TokenKind::LBracket) {
                let index = self.parse_expr()?;
                self.expect(TokenKind::RBracket, "Expected ']' after index")?;
                expr = Expr::Index {
                    collection: Box::new(expr),
                    index: Box::new(index),
                };
                continue;
            }
            if self.matches(&TokenKind::LParen) {
                let mut args = Vec::new();
                if !matches!(self.peek_kind(), TokenKind::RParen) {
                    loop {
                        let arg = self.parse_expr()?;
                        args.push(arg);
                        if self.matches(&TokenKind::Comma) {
                            if matches!(self.peek_kind(), TokenKind::RParen) {
                                break;
                            }
                            continue;
                        }
                        break;
                    }
                }
                self.expect(TokenKind::RParen, "Expected ')' after arguments")?;
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_map_literal(&mut self) -> Result<Expr, ParseError> {
        let mut entries = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RBrace) {
            loop {
                let key = match self.advance().kind.clone() {
                    TokenKind::String(value) => value,
                    other => {
                        return Err(ParseError {
                            message: format!(
                                "Expected string literal key in map, found {other:?}"
                            ),
                            span: self.previous_span(),
                        })
                    }
                };
                self.expect(TokenKind::Colon, "Expected ':' after map key")?;
                let value = self.parse_expr()?;
                entries.push((key, value));
                if self.matches(&TokenKind::Comma) {
                    if matches!(self.peek_kind(), TokenKind::RBrace) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        self.expect(TokenKind::RBrace, "Expected '}' after map literal")?;
        Ok(Expr::MapLiteral(entries))
    }

    fn parse_set_literal(&mut self) -> Result<Expr, ParseError> {
        let mut entries = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RBrace) {
            loop {
                let value = self.parse_expr()?;
                entries.push(value);
                if self.matches(&TokenKind::Comma) {
                    if matches!(self.peek_kind(), TokenKind::RBrace) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        self.expect(TokenKind::RBrace, "Expected '}' after set literal")?;
        Ok(Expr::SetLiteral(entries))
    }

    fn lookahead_is_set_literal(&mut self) -> Result<bool, ParseError> {
        let mut depth = 1usize;
        let mut i = self.index;
        while let Some(token) = self.tokens.get(i) {
            match &token.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::Colon if depth == 1 => return Ok(false),
                _ => {}
            }
            i += 1;
        }
        Ok(true)
    }

    fn parse_assign_stmt(&mut self) -> Result<Stmt, ParseError> {
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(name) => name,
            other => {
                return Err(ParseError {
                    message: format!("Expected identifier in assignment, found {other:?}"),
                    span: self.previous_span(),
                })
            }
        };
        self.expect(TokenKind::Equal, "Expected '=' in assignment")?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::Semicolon, "Expected ';' after assignment")?;
        Ok(Stmt::Assign { name, value })
    }

    fn is_assignment(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Ident(_))
            && matches!(self.peek_next_kind(), Some(TokenKind::Equal))
    }

    fn peek_next_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.index + 1).map(|token| &token.kind)
    }

    fn is_range_terminator(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Semicolon | TokenKind::RParen | TokenKind::RBrace
        )
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
