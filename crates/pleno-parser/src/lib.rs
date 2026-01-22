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
        let annotations = self.parse_annotations()?;
        match self.peek_kind() {
            TokenKind::KeywordModule => self.parse_module_stmt(),
            TokenKind::KeywordLet => self.parse_let_stmt(),
            TokenKind::KeywordConst => self.parse_const_stmt(),
            TokenKind::KeywordIf => self.parse_if_stmt(),
            TokenKind::KeywordWhile => self.parse_while_stmt(),
            TokenKind::KeywordFor => self.parse_for_stmt(),
            TokenKind::KeywordImport => self.parse_import_stmt(),
            TokenKind::KeywordExport => self.parse_export_stmt(),
            TokenKind::KeywordClass => {
                let stmt = self.parse_class_stmt()?;
                if let Stmt::Class {
                    name,
                    fields,
                    methods,
                    superclass,
                    ..
                } = stmt
                {
                    Ok(Stmt::Class {
                        name,
                        fields,
                        methods,
                        superclass,
                        annotations,
                    })
                } else {
                    Ok(stmt)
                }
            }
            TokenKind::KeywordFn => {
                let decl = self.parse_function_decl(
                    pleno_ast::FunctionKind::Method,
                    false,
                    None,
                    None,
                    annotations,
                )?;
                Ok(Stmt::Fn {
                    name: decl.name,
                    params: decl.params,
                    body: decl.body,
                    annotations: decl.annotations,
                })
            }
            TokenKind::KeywordReturn => self.parse_return_stmt(),
            TokenKind::KeywordBreak => self.parse_break_stmt(),
            TokenKind::KeywordContinue => self.parse_continue_stmt(),
            TokenKind::LBrace => self.parse_block_stmt(),
            _ => {
                if !annotations.is_empty() {
                    return Err(ParseError {
                        message: "Annotations are only allowed on functions, classes, and fields"
                            .to_string(),
                        span: self.previous_span(),
                    });
                }
                let expr = self.parse_expr()?;
                if self.matches(&TokenKind::Equal) {
                    let value = self.parse_expr()?;
                    self.expect(TokenKind::Semicolon, "Expected ';' after assignment")?;
                    return match expr {
                        Expr::Ident(name) => Ok(Stmt::Assign { name, value }),
                        Expr::Property { object, name } => Ok(Stmt::AssignProperty {
                            object: *object,
                            name,
                            value,
                        }),
                        Expr::Index { collection, index } => Ok(Stmt::AssignIndex {
                            collection: *collection,
                            index: *index,
                            value,
                        }),
                        _ => Err(ParseError {
                            message: "Invalid assignment target".to_string(),
                            span: self.previous_span(),
                        }),
                    };
                }
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

    fn parse_module_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected module name, found {other:?}"),
                    span: self.previous_span(),
                })
            }
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after module declaration")?;
        Ok(Stmt::Module { name })
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
        let decl = self.parse_function_decl(pleno_ast::FunctionKind::Method, false, None, None)?;
        Ok(Stmt::Fn {
            name: decl.name,
            params: decl.params,
            body: decl.body,
        })
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

    fn parse_import_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let kind = match self.peek_kind() {
            TokenKind::LBrace => {
                self.advance();
                let mut items = Vec::new();
                while !matches!(self.peek_kind(), TokenKind::RBrace) {
                    let name = match self.advance().kind.clone() {
                        TokenKind::Ident(value) => value,
                        other => {
                            return Err(ParseError {
                                message: format!("Expected import name, found {other:?}"),
                                span: self.previous_span(),
                            })
                        }
                    };
                    let alias = if self.matches(&TokenKind::KeywordAs) {
                        match self.advance().kind.clone() {
                            TokenKind::Ident(value) => Some(value),
                            other => {
                                return Err(ParseError {
                                    message: format!(
                                        "Expected alias name after 'as', found {other:?}"
                                    ),
                                    span: self.previous_span(),
                                })
                            }
                        }
                    } else {
                        None
                    };
                    items.push(pleno_ast::ImportName { name, alias });
                    if !self.matches(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RBrace, "Expected '}' after import list")?;
                self.expect(TokenKind::KeywordFrom, "Expected 'from' after import list")?;
                pleno_ast::ImportKind::Names(items)
            }
            TokenKind::Star => {
                self.advance();
                let alias = if self.matches(&TokenKind::KeywordAs) {
                    match self.advance().kind.clone() {
                        TokenKind::Ident(value) => value,
                        other => {
                            return Err(ParseError {
                                message: format!("Expected alias after 'as', found {other:?}"),
                                span: self.previous_span(),
                            })
                        }
                    }
                } else {
                    return Err(ParseError {
                        message: "Expected 'as' alias after '*' import".to_string(),
                        span: self.previous_span(),
                    });
                };
                self.expect(TokenKind::KeywordFrom, "Expected 'from' after import alias")?;
                pleno_ast::ImportKind::Star { alias }
            }
            TokenKind::Ident(_) => {
                let alias = match self.advance().kind.clone() {
                    TokenKind::Ident(name) => name,
                    _ => unreachable!(),
                };
                self.expect(TokenKind::KeywordFrom, "Expected 'from' in import")?;
                pleno_ast::ImportKind::Module { alias: Some(alias) }
            }
            _ => pleno_ast::ImportKind::Module { alias: None },
        };
        let path = match self.advance().kind.clone() {
            TokenKind::String(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected string literal path, found {other:?}"),
                    span: self.previous_span(),
                })
            }
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after import")?;
        Ok(Stmt::Import { path, kind })
    }

    fn parse_export_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        if self.matches(&TokenKind::LBrace) {
            let mut items = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::RBrace) {
                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(ParseError {
                            message: format!("Expected export name, found {other:?}"),
                            span: self.previous_span(),
                        })
                    }
                };
                let alias = if self.matches(&TokenKind::KeywordAs) {
                    match self.advance().kind.clone() {
                        TokenKind::Ident(value) => Some(value),
                        other => {
                            return Err(ParseError {
                                message: format!(
                                    "Expected alias name after 'as', found {other:?}"
                                ),
                                span: self.previous_span(),
                            })
                        }
                    }
                } else {
                    None
                };
                items.push(pleno_ast::ExportName { name, alias });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RBrace, "Expected '}' after export list")?;
            self.expect(TokenKind::Semicolon, "Expected ';' after export list")?;
            return Ok(Stmt::ExportList(items));
        }
        let stmt = match self.peek_kind() {
            TokenKind::KeywordFn => self.parse_fn_stmt()?,
            TokenKind::KeywordClass => self.parse_class_stmt()?,
            TokenKind::KeywordLet => self.parse_let_stmt()?,
            TokenKind::KeywordConst => self.parse_const_stmt()?,
            other => {
                return Err(ParseError {
                    message: format!("Expected declaration after export, found {other:?}"),
                    span: self.peek().span,
                })
            }
        };
        Ok(Stmt::Export(Box::new(stmt)))
    }

    fn parse_class_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.advance();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected class name, found {other:?}"),
                    span: self.previous_span(),
                })
            }
        };
        let superclass = if self.matches(&TokenKind::KeywordExtends) {
            match self.advance().kind.clone() {
                TokenKind::Ident(value) => Some(value),
                other => {
                    return Err(ParseError {
                        message: format!("Expected superclass name, found {other:?}"),
                        span: self.previous_span(),
                    })
                }
            }
        } else {
            None
        };
        self.expect(TokenKind::LBrace, "Expected '{' after class name")?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !self.is_at_end() && !matches!(self.peek_kind(), TokenKind::RBrace) {
            let annotations = self.parse_annotations()?;
            let mut visibility = None;
            let mut is_static = false;
            loop {
                match self.peek_kind() {
                    TokenKind::KeywordPublic => {
                        self.advance();
                        visibility = Some(pleno_ast::Visibility::Public);
                    }
                    TokenKind::KeywordPrivate => {
                        self.advance();
                        visibility = Some(pleno_ast::Visibility::Private);
                    }
                    TokenKind::KeywordProtected => {
                        self.advance();
                        visibility = Some(pleno_ast::Visibility::Protected);
                    }
                    TokenKind::KeywordInternal => {
                        self.advance();
                        visibility = Some(pleno_ast::Visibility::Internal);
                    }
                    TokenKind::KeywordStatic => {
                        self.advance();
                        is_static = true;
                    }
                    _ => break,
                }
            }
            if matches!(self.peek_kind(), TokenKind::KeywordFn) {
                methods.push(self.parse_function_decl(
                    pleno_ast::FunctionKind::Method,
                    true,
                    visibility,
                    Some(is_static),
                    annotations,
                )?);
                continue;
            }
            if is_accessor(self.peek_kind(), self.peek_n_kind(1), self.peek_n_kind(2), "get") {
                methods.push(self.parse_accessor_decl(
                    pleno_ast::FunctionKind::Getter,
                    visibility,
                    is_static,
                    annotations,
                )?);
                continue;
            }
            if is_accessor(self.peek_kind(), self.peek_n_kind(1), self.peek_n_kind(2), "set") {
                methods.push(self.parse_accessor_decl(
                    pleno_ast::FunctionKind::Setter,
                    visibility,
                    is_static,
                    annotations,
                )?);
                continue;
            }
            let name = match self.advance().kind.clone() {
                TokenKind::Ident(value) => value,
                other => {
                    return Err(ParseError {
                        message: format!("Expected field name in class body, found {other:?}"),
                        span: self.previous_span(),
                    });
                }
            };
            self.expect(TokenKind::Semicolon, "Expected ';' after field declaration")?;
            fields.push(pleno_ast::FieldDecl {
                name,
                is_static,
                visibility,
                annotations,
            });
        }
        self.expect(TokenKind::RBrace, "Expected '}' after class body")?;
        Ok(Stmt::Class {
            name,
            fields,
            methods,
            superclass,
        })
    }

    fn parse_function_decl(
        &mut self,
        kind: pleno_ast::FunctionKind,
        allow_modifiers: bool,
        visibility: Option<pleno_ast::Visibility>,
        is_static: Option<bool>,
        annotations: Vec<pleno_ast::Annotation>,
    ) -> Result<pleno_ast::FunctionDecl, ParseError> {
        self.advance();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(name) => name,
            other => {
                return Err(ParseError {
                    message: format!("Expected function name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        self.expect(TokenKind::LParen, "Expected '(' after function name")?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen, "Expected ')' after parameters")?;
        let body = Box::new(self.parse_block_stmt()?);
        let (visibility, is_static) = if allow_modifiers {
            (visibility, is_static.unwrap_or(false))
        } else {
            if visibility.is_some() || is_static.unwrap_or(false) {
                return Err(ParseError {
                    message: "Function modifiers are only allowed in classes".to_string(),
                    span: self.previous_span(),
                });
            }
            (None, false)
        };
        Ok(pleno_ast::FunctionDecl {
            name,
            params,
            body,
            is_static,
            visibility,
            kind,
            annotations,
        })
    }

    fn parse_accessor_decl(
        &mut self,
        kind: pleno_ast::FunctionKind,
        visibility: Option<pleno_ast::Visibility>,
        is_static: bool,
        annotations: Vec<pleno_ast::Annotation>,
    ) -> Result<pleno_ast::FunctionDecl, ParseError> {
        self.advance();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected accessor name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        self.expect(TokenKind::LParen, "Expected '(' after accessor name")?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen, "Expected ')' after accessor parameters")?;
        match kind {
            pleno_ast::FunctionKind::Getter if !params.is_empty() => {
                return Err(ParseError {
                    message: "getter cannot take parameters".to_string(),
                    span: self.previous_span(),
                });
            }
            pleno_ast::FunctionKind::Setter if params.len() != 1 => {
                return Err(ParseError {
                    message: "setter must take exactly one parameter".to_string(),
                    span: self.previous_span(),
                });
            }
            _ => {}
        }
        let body = self.parse_block_stmt()?;
        Ok(pleno_ast::FunctionDecl {
            name,
            params,
            body: Box::new(body),
            is_static,
            visibility,
            kind,
            annotations,
        })
    }

    fn parse_annotations(&mut self) -> Result<Vec<pleno_ast::Annotation>, ParseError> {
        let mut annotations = Vec::new();
        while self.matches(&TokenKind::At) {
            annotations.push(self.parse_annotation()?);
        }
        Ok(annotations)
    }

    fn parse_annotation(&mut self) -> Result<pleno_ast::Annotation, ParseError> {
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(name) => name,
            other => {
                return Err(ParseError {
                    message: format!("Expected annotation name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        let mut args = Vec::new();
        if self.matches(&TokenKind::LParen) {
            if !matches!(self.peek_kind(), TokenKind::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if !self.matches(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RParen, "Expected ')' after annotation arguments")?;
        }
        Ok(pleno_ast::Annotation { name, args })
    }

    fn parse_param_list(&mut self) -> Result<Vec<pleno_ast::Param>, ParseError> {
        let mut params = Vec::new();
        let mut keyword_only = false;
        let mut seen_variadic = false;
        if matches!(self.peek_kind(), TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            if self.matches(&TokenKind::Star) {
                if keyword_only {
                    return Err(ParseError {
                        message: "duplicate keyword-only marker".to_string(),
                        span: self.previous_span(),
                    });
                }
                keyword_only = true;
                if self.matches(&TokenKind::Comma) {
                    continue;
                }
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    break;
                }
            }
            let is_variadic = if self.matches(&TokenKind::Ellipsis) {
                if seen_variadic {
                    return Err(ParseError {
                        message: "variadic parameter already defined".to_string(),
                        span: self.previous_span(),
                    });
                }
                seen_variadic = true;
                true
            } else {
                false
            };
            let name = match self.advance().kind.clone() {
                TokenKind::Ident(name) => name,
                other => {
                    return Err(ParseError {
                        message: format!("Expected parameter name, found {other:?}"),
                        span: self.previous_span(),
                    })
                }
            };
            let default = if self.matches(&TokenKind::Equal) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            if is_variadic && default.is_some() {
                return Err(ParseError {
                    message: "variadic parameter cannot have default".to_string(),
                    span: self.previous_span(),
                });
            }
            params.push(pleno_ast::Param {
                name,
                default,
                is_variadic,
                is_keyword_only: keyword_only,
            });
            if is_variadic && !matches!(self.peek_kind(), TokenKind::RParen) {
                return Err(ParseError {
                    message: "variadic parameter must be last".to_string(),
                    span: self.peek().span,
                });
            }
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
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
            TokenKind::Underscore => Expr::Underscore,
            TokenKind::Ident(name) => Expr::Ident(name),
            TokenKind::KeywordThis => Expr::This,
            TokenKind::KeywordSuper => Expr::Super,
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
                        let arg = if matches!(self.peek_kind(), TokenKind::Underscore) {
                            self.advance();
                            pleno_ast::CallArg::Placeholder
                        } else if matches!(self.peek_kind(), TokenKind::Ident(_))
                            && matches!(self.peek_next_kind(), Some(TokenKind::Equal))
                        {
                            let name = match self.advance().kind.clone() {
                                TokenKind::Ident(value) => value,
                                _ => unreachable!("ident checked"),
                            };
                            self.expect(TokenKind::Equal, "Expected '=' in named argument")?;
                            let value = self.parse_expr()?;
                            pleno_ast::CallArg::Named(name, value)
                        } else {
                            pleno_ast::CallArg::Positional(self.parse_expr()?)
                        };
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

    fn peek_next_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.index + 1).map(|token| &token.kind)
    }

    fn peek_n_kind(&self, offset: usize) -> Option<&TokenKind> {
        self.tokens.get(self.index + offset).map(|token| &token.kind)
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

fn is_accessor(
    first: &TokenKind,
    second: Option<&TokenKind>,
    third: Option<&TokenKind>,
    name: &str,
) -> bool {
    match (first, second, third) {
        (TokenKind::Ident(ident), Some(TokenKind::Ident(_)), Some(TokenKind::LParen))
            if ident == name =>
        {
            true
        }
        _ => false,
    }
}
