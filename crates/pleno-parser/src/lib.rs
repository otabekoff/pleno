use pleno_ast::{Comment, Expr, ExprKind, Literal, Program, Span, Stmt, StmtKind};
use pleno_lexer::{Token, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BracedKind {
    Set,
    Map,
    Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

pub struct Parser {
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    index: usize,
}

struct FunctionDeclarationConfig {
    kind: pleno_ast::FunctionKind,
    allow_modifiers: bool,
    visibility: Option<pleno_ast::Visibility>,
    is_static: Option<bool>,
    annotations: Vec<pleno_ast::Annotation>,
    is_async: bool,
    is_abstract: bool,
}

impl Parser {
    pub fn new(raw_tokens: Vec<Token>) -> Self {
        let mut tokens = Vec::new();
        let mut comments = Vec::new();

        for token in raw_tokens {
            if let TokenKind::Comment(content, kind) = token.kind {
                comments.push(Comment {
                    span: token.span,
                    content,
                    kind,
                });
            } else {
                tokens.push(token);
            }
        }

        Self {
            tokens,
            comments,
            index: 0,
        }
    }

    fn peek_span(&self) -> Span {
        if self.index < self.tokens.len() {
            self.tokens[self.index].span
        } else {
            self.tokens
                .last()
                .map(|t| t.span)
                .unwrap_or(Span::new(0, 0))
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            if self.peek_kind() == &TokenKind::Eof {
                break;
            }
            statements.push(self.parse_stmt()?);
        }
        Ok(Program::new(statements, self.comments.clone()))
    }

    fn wrap_expr(&self, kind: ExprKind, start: Span) -> Expr {
        Expr::new(kind, Span::new(start.start, self.previous_span().end))
    }

    fn wrap_stmt(&self, kind: StmtKind, start: Span) -> Stmt {
        Stmt::new(kind, Span::new(start.start, self.previous_span().end))
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        let annotations = self.parse_annotations()?;

        // Check for labeled statement: label: stmt
        if let TokenKind::Ident(label) = self.peek_kind()
            && matches!(self.peek_next_kind(), Some(TokenKind::Colon))
        {
            let label = label.clone();
            self.advance(); // consume ident
            self.advance(); // consume colon
            let stmt = Box::new(self.parse_stmt()?);
            return Ok(self.wrap_stmt(StmtKind::Labeled { label, stmt }, start_span));
        }

        match self.peek_kind() {
            TokenKind::KeywordModule => self.parse_module_stmt(),
            TokenKind::KeywordLet => self.parse_let_stmt(),
            TokenKind::KeywordConst => self.parse_const_stmt(),
            TokenKind::KeywordMacro => self.parse_macro_decl_stmt(),
            TokenKind::KeywordIf => self.parse_if_stmt(),
            TokenKind::KeywordWhile => self.parse_while_stmt(),
            TokenKind::KeywordFor => self.parse_for_stmt(),
            TokenKind::KeywordTry => self.parse_try_stmt(),
            TokenKind::KeywordThrow => self.parse_throw_stmt(),
            TokenKind::KeywordImport => self.parse_import_stmt(),
            TokenKind::KeywordExport => self.parse_export_stmt(annotations),
            TokenKind::KeywordEnum => self.parse_enum_declaration(annotations),
            TokenKind::KeywordExtend => self.parse_extend_stmt(),
            TokenKind::KeywordClass => self.parse_class_stmt(annotations, false),
            TokenKind::KeywordAbstract => {
                self.advance();
                match self.peek_kind() {
                    TokenKind::KeywordClass => self.parse_class_stmt(annotations, true),
                    other => Err(ParseError {
                        message: format!("Expected 'class' after 'abstract', found {other:?}"),
                        span: self.peek_span(),
                    }),
                }
            }
            TokenKind::KeywordInterface => self.parse_interface_stmt(annotations),
            TokenKind::KeywordType => self.parse_type_alias_stmt(),
            TokenKind::KeywordDo => self.parse_do_while_stmt(),
            TokenKind::KeywordAsync => {
                if matches!(self.peek_n_kind(1), Some(TokenKind::KeywordFn)) {
                    self.advance(); // consume 'async'
                    let decl = self.parse_function_decl(FunctionDeclarationConfig {
                        kind: pleno_ast::FunctionKind::Method,
                        allow_modifiers: false,
                        visibility: None,
                        is_static: None,
                        annotations,
                        is_async: true,
                        is_abstract: false,
                    })?;
                    Ok(self.wrap_stmt(
                        StmtKind::Fn {
                            name: decl.name,
                            generics: decl.generics,
                            params: decl.params,
                            body: decl.body,
                            annotations: decl.annotations,
                            is_async: true,
                        },
                        start_span,
                    ))
                } else {
                    Err(ParseError {
                        message: "Expected 'fn' after 'async'".to_string(),
                        span: self.previous_span(),
                    })
                }
            }
            TokenKind::KeywordFn => {
                let decl = self.parse_function_decl(FunctionDeclarationConfig {
                    kind: pleno_ast::FunctionKind::Method,
                    allow_modifiers: false,
                    visibility: None,
                    is_static: None,
                    annotations,
                    is_async: false,
                    is_abstract: false,
                })?;
                Ok(self.wrap_stmt(
                    StmtKind::Fn {
                        name: decl.name,
                        generics: decl.generics,
                        params: decl.params,
                        body: decl.body,
                        annotations: decl.annotations,
                        is_async: false,
                    },
                    start_span,
                ))
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
                    return match expr.kind {
                        ExprKind::Ident(name) => {
                            Ok(self.wrap_stmt(StmtKind::Assign { name, value }, start_span))
                        }
                        ExprKind::Property { object, name } => Ok(self.wrap_stmt(
                            StmtKind::AssignProperty {
                                object: *object,
                                name,
                                value,
                            },
                            start_span,
                        )),
                        ExprKind::Index { collection, index } => Ok(self.wrap_stmt(
                            StmtKind::AssignIndex {
                                collection: *collection,
                                index: *index,
                                value,
                            },
                            start_span,
                        )),
                        _ => Err(ParseError {
                            message: "Invalid assignment target".to_string(),
                            span: self.previous_span(),
                        }),
                    };
                }
                if self.matches(&TokenKind::Semicolon)
                    || matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof)
                    || matches!(expr.kind, ExprKind::Match { .. } | ExprKind::Block(_))
                {
                    Ok(self.wrap_stmt(StmtKind::Expr(expr), start_span))
                } else {
                    Err(ParseError {
                        message: "Expected ';' after expression".to_string(),
                        span: self.previous_span(),
                    })
                }
            }
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let stmt = self.parse_let_declaration(false)?;
        self.expect(TokenKind::Semicolon, "Expected ';' after let statement")?;
        Ok(stmt)
    }

    fn parse_let_declaration(&mut self, forced_mutable: bool) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // let
        let is_lazy = self.matches(&TokenKind::KeywordLazy);
        let mutable = self.matches(&TokenKind::KeywordMut) || forced_mutable;
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected identifier after 'let', found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        let ty = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let value = if self.matches(&TokenKind::Equal) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(self.wrap_stmt(
            StmtKind::Let {
                name,
                mutable,
                ty,
                value,
                is_const: false,
                is_deep: false,
                is_lazy,
            },
            start_span,
        ))
    }

    fn parse_const_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // consume const
        if self.matches(&TokenKind::KeywordMut) {
            return Err(ParseError {
                message: "const cannot be mutable".to_string(),
                span: self.previous_span(),
            });
        }

        let is_deep = self.matches(&TokenKind::KeywordDeep);

        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected identifier after 'const', found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        let ty = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let value = if self.matches(&TokenKind::Equal) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after const statement")?;
        Ok(self.wrap_stmt(
            StmtKind::Let {
                name,
                mutable: false,
                ty,
                value,
                is_const: true,
                is_deep,
                is_lazy: false,
            },
            start_span,
        ))
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
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
        Ok(self.wrap_stmt(
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            },
            start_span,
        ))
    }

    fn parse_expr_or_assign(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        let expr = self.parse_expr()?;
        if self.matches(&TokenKind::Equal) {
            let value = self.parse_expr()?;
            return match expr.kind {
                ExprKind::Ident(name) => {
                    Ok(self.wrap_stmt(StmtKind::Assign { name, value }, start_span))
                }
                ExprKind::Property { object, name } => Ok(self.wrap_stmt(
                    StmtKind::AssignProperty {
                        object: *object,
                        name,
                        value,
                    },
                    start_span,
                )),
                ExprKind::Index { collection, index } => Ok(self.wrap_stmt(
                    StmtKind::AssignIndex {
                        collection: *collection,
                        index: *index,
                        value,
                    },
                    start_span,
                )),
                _ => Err(ParseError {
                    message: "Invalid assignment target".to_string(),
                    span: expr.span,
                }),
            };
        }
        Ok(self.wrap_stmt(StmtKind::Expr(expr), start_span))
    }

    fn parse_module_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected module name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };

        if self.matches(&TokenKind::Semicolon) {
            Ok(self.wrap_stmt(StmtKind::Module { name, body: None }, start_span))
        } else {
            self.expect(TokenKind::LBrace, "Expected '{' or ';' after module name")?;
            let mut body = Vec::new();
            while !self.matches(&TokenKind::RBrace) {
                if self.is_at_end() {
                    return Err(ParseError {
                        message: "Unterminated module block".to_string(),
                        span: self.peek_span(),
                    });
                }
                body.push(self.parse_stmt()?);
            }
            Ok(self.wrap_stmt(
                StmtKind::Module {
                    name,
                    body: Some(body),
                },
                start_span,
            ))
        }
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        self.expect(TokenKind::LParen, "Expected '(' after 'while'")?;
        let condition = self.parse_expr()?;
        self.expect(TokenKind::RParen, "Expected ')' after while condition")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(self.wrap_stmt(StmtKind::While { condition, body }, start_span))
    }

    fn parse_macro_decl_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // macro
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected macro name after 'macro', found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        self.expect(TokenKind::LParen, "Expected '(' after macro name")?;
        let mut params = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RParen) {
            loop {
                let param = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(ParseError {
                            message: format!(
                                "Expected parameter name in macro declaration, found {other:?}"
                            ),
                            span: self.previous_span(),
                        });
                    }
                };
                params.push(param);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen, "Expected ')' after macro parameters")?;
        let body = Box::new(self.parse_stmt()?);
        Ok(self.wrap_stmt(StmtKind::MacroDecl { name, params, body }, start_span))
    }

    fn parse_fn_stmt(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        let decl = self.parse_function_decl(FunctionDeclarationConfig {
            kind: pleno_ast::FunctionKind::Method,
            allow_modifiers: false,
            visibility: None,
            is_static: None,
            annotations,
            is_async: false,
            is_abstract: false,
        })?;
        Ok(self.wrap_stmt(
            StmtKind::Fn {
                name: decl.name,
                generics: decl.generics,
                params: decl.params,
                body: decl.body,
                annotations: decl.annotations,
                is_async: false,
            },
            start_span,
        ))
    }

    fn parse_return_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let expr = if matches!(self.peek_kind(), TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after return")?;
        Ok(self.wrap_stmt(StmtKind::Return(expr), start_span))
    }

    fn parse_break_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let label = if let TokenKind::Ident(label) = self.peek_kind() {
            let label = label.clone();
            self.advance();
            Some(label)
        } else {
            None
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after break")?;
        Ok(self.wrap_stmt(StmtKind::Break(label), start_span))
    }

    fn parse_continue_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let label = if let TokenKind::Ident(label) = self.peek_kind() {
            let label = label.clone();
            self.advance();
            Some(label)
        } else {
            None
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after continue")?;
        Ok(self.wrap_stmt(StmtKind::Continue(label), start_span))
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // 'for'
        self.expect(TokenKind::LParen, "Expected '(' after 'for'")?;

        // Try to parse C-style for or range for
        // If we see ';' immediately, it's C-style: for (; ...; ...)
        if self.check(&TokenKind::Semicolon) {
            return self.parse_c_for_remainder(None, start_span);
        }

        // If we see 'let', 'mut', or 'const', it might be C-style declaration
        // for (let i = 0; i < 10; i++)
        if self.check(&TokenKind::KeywordLet) {
            let init_stmt = self.parse_let_declaration(true)?; // C-style loop variables are mutable
            return self.parse_c_for_remainder(Some(Box::new(init_stmt)), start_span);
        }

        // Handle mut i = 0 or const i = 0 if they don't use 'let' but are decls
        // (Wait, in Pleno 'mut' and 'const' usually follow 'let' or are modifiers)
        // Let's stick to 'let' for now as 'parse_let_declaration' handles 'let mut'
        // If it's a raw assignment, it's an expression.

        // Otherwise, it could be range or C-style expression init
        // We peek ahead to see if 'in' exists before ';'
        let mut is_range = false;
        let mut i = 0;
        while !self.is_at_end()
            && self.peek_n_kind(i).is_some()
            && self.peek_n_kind(i) != Some(&TokenKind::RParen)
        {
            if self.peek_n_kind(i) == Some(&TokenKind::KeywordIn) {
                is_range = true;
                break;
            }
            if self.peek_n_kind(i) == Some(&TokenKind::Semicolon) {
                break;
            }
            i += 1;
        }

        if is_range {
            let mut names = Vec::new();
            loop {
                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(name) => name,
                    other => {
                        return Err(ParseError {
                            message: format!("Expected identifier in for loop, found {other:?}"),
                            span: self.previous_span(),
                        });
                    }
                };
                names.push(name);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::KeywordIn, "Expected 'in' in for loop")?;
            let iterable = self.parse_expr()?;
            self.expect(TokenKind::RParen, "Expected ')' after for loop")?;
            let body = Box::new(self.parse_stmt()?);
            Ok(self.wrap_stmt(
                StmtKind::For {
                    label: None,
                    names,
                    iterable,
                    body,
                },
                start_span,
            ))
        } else {
            let init = self.parse_expr_or_assign()?;
            self.parse_c_for_remainder(Some(Box::new(init)), start_span)
        }
    }

    fn parse_c_for_remainder(
        &mut self,
        init: Option<Box<Stmt>>,
        start_span: Span,
    ) -> Result<Stmt, ParseError> {
        self.expect(
            TokenKind::Semicolon,
            "Expected ';' after loop initialization",
        )?;

        let condition = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after loop condition")?;

        let step = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(Box::new(self.parse_expr_or_assign()?))
        };
        self.expect(TokenKind::RParen, "Expected ')' after for clause")?;

        let body = Box::new(self.parse_stmt()?);

        Ok(self.wrap_stmt(
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            },
            start_span,
        ))
    }

    fn parse_extend_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // extend

        let generics = if self.check(&TokenKind::Less) {
            self.parse_generic_params()?
        } else {
            Vec::new()
        };

        let target = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected extension target name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };

        self.expect(TokenKind::LBrace, "Expected '{' after extension target")?;

        let mut methods = Vec::new();
        while !self.is_at_end() && !matches!(self.peek_kind(), TokenKind::RBrace) {
            let member_annotations = self.parse_annotations()?;
            let decl = self.parse_function_decl(FunctionDeclarationConfig {
                kind: pleno_ast::FunctionKind::Method,
                allow_modifiers: true,
                visibility: None,
                is_static: None,
                annotations: member_annotations,
                is_async: false,
                is_abstract: false,
            })?;
            methods.push(decl);
        }

        self.expect(TokenKind::RBrace, "Expected '}' after extension body")?;

        Ok(self.wrap_stmt(
            StmtKind::Extend {
                target,
                generics,
                methods,
            },
            start_span,
        ))
    }

    fn parse_interface_stmt(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // interface
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected interface name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };

        let generics = self.parse_generic_params()?;

        let mut extends = Vec::new();
        if self.matches(&TokenKind::KeywordExtends) {
            loop {
                let parent = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(ParseError {
                            message: format!("Expected extended interface name, found {other:?}"),
                            span: self.previous_span(),
                        });
                    }
                };
                extends.push(parent);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }

        self.expect(TokenKind::LBrace, "Expected '{' before interface body")?;
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let method_annotations = self.parse_annotations()?;
            let method = self.parse_function_decl(FunctionDeclarationConfig {
                kind: pleno_ast::FunctionKind::Method,
                allow_modifiers: true,
                visibility: None,
                is_static: None,
                annotations: method_annotations,
                is_async: false,
                is_abstract: true,
            })?;
            methods.push(method);
        }
        self.expect(TokenKind::RBrace, "Expected '}' after interface body")?;

        Ok(self.wrap_stmt(
            StmtKind::Interface {
                name,
                generics,
                methods,
                extends,
                annotations,
            },
            start_span,
        ))
    }

    fn parse_enum_declaration(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // enum
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected enum name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };

        // Parse optional generic parameters
        let _generics = self.parse_generic_params()?;

        self.expect(TokenKind::LBrace, "Expected '{' before enum body")?;
        let mut variants = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let variant_name = match self.advance().kind.clone() {
                TokenKind::Ident(value) => value,
                other => {
                    return Err(ParseError {
                        message: format!("Expected variant name, found {other:?}"),
                        span: self.previous_span(),
                    });
                }
            };

            // Parse optional parameters for this variant
            let params = if self.matches(&TokenKind::LParen) {
                let mut params = Vec::new();
                while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                    let param_name = match self.advance().kind.clone() {
                        TokenKind::Ident(value) => value,
                        other => {
                            return Err(ParseError {
                                message: format!("Expected parameter name, found {other:?}"),
                                span: self.previous_span(),
                            });
                        }
                    };
                    let ty = if self.matches(&TokenKind::Colon) {
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    params.push(pleno_ast::Param {
                        name: param_name,
                        ty,
                        default: None,
                        is_variadic: false,
                        is_keyword_only: false,
                    });
                    if !self.matches(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen, "Expected ')' after variant parameters")?;
                Some(params)
            } else {
                None
            };

            variants.push(pleno_ast::EnumVariant {
                name: variant_name,
                params,
            });

            // Allow trailing comma
            self.matches(&TokenKind::Comma);
        }
        self.expect(TokenKind::RBrace, "Expected '}' after enum body")?;

        Ok(self.wrap_stmt(
            StmtKind::Enum(pleno_ast::EnumDecl {
                name,
                variants,
                annotations,
            }),
            start_span,
        ))
    }

    fn parse_do_while_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // do
        let body = Box::new(self.parse_stmt()?);
        self.expect(TokenKind::KeywordWhile, "Expected 'while' after do block")?;
        self.expect(TokenKind::LParen, "Expected '(' after while")?;
        let condition = self.parse_expr()?;
        self.expect(TokenKind::RParen, "Expected ')' after condition")?;
        self.expect(TokenKind::Semicolon, "Expected ';' after do-while")?;
        Ok(self.wrap_stmt(StmtKind::DoWhile { body, condition }, start_span))
    }

    fn parse_try_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let try_branch = Box::new(self.parse_stmt()?);
        let mut catch_branches = Vec::new();
        while self.matches(&TokenKind::KeywordCatch) {
            self.expect(TokenKind::LParen, "Expected '(' after 'catch'")?;
            let name = match self.advance().kind.clone() {
                TokenKind::Ident(name) => name,
                other => {
                    return Err(ParseError {
                        message: format!("Expected identifier in catch, found {other:?}"),
                        span: self.previous_span(),
                    });
                }
            };
            let ty = if self.matches(&TokenKind::Colon) {
                match self.advance().kind.clone() {
                    TokenKind::Ident(ty) => Some(ty),
                    other => {
                        return Err(ParseError {
                            message: format!("Expected type name in catch, found {other:?}"),
                            span: self.previous_span(),
                        });
                    }
                }
            } else {
                None
            };
            self.expect(TokenKind::RParen, "Expected ')' after catch parameters")?;
            let body = Box::new(self.parse_stmt()?);
            catch_branches.push(pleno_ast::CatchBlock { name, ty, body });
        }
        let finally_branch = if self.matches(&TokenKind::KeywordFinally) {
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        if catch_branches.is_empty() && finally_branch.is_none() {
            return Err(ParseError {
                message: "try statement must have at least one catch or finally block".to_string(),
                span: self.previous_span(),
            });
        }
        Ok(self.wrap_stmt(
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            },
            start_span,
        ))
    }

    fn parse_throw_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let expr = self.parse_expr()?;
        self.expect(TokenKind::Semicolon, "Expected ';' after throw")?;
        Ok(self.wrap_stmt(StmtKind::Throw(expr), start_span))
    }

    fn parse_import_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
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
                            });
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
                                });
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
                            });
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
                });
            }
        };
        self.expect(TokenKind::Semicolon, "Expected ';' after import")?;
        Ok(self.wrap_stmt(StmtKind::Import { path, kind }, start_span))
    }

    fn parse_export_stmt(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        if self.matches(&TokenKind::LBrace) {
            if !annotations.is_empty() {
                return Err(ParseError {
                    message: "Annotations are not allowed on export lists".to_string(),
                    span: self.previous_span(),
                });
            }
            let mut items = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::RBrace) {
                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(ParseError {
                            message: format!("Expected export name, found {other:?}"),
                            span: self.previous_span(),
                        });
                    }
                };
                let alias = if self.matches(&TokenKind::KeywordAs) {
                    match self.advance().kind.clone() {
                        TokenKind::Ident(value) => Some(value),
                        other => {
                            return Err(ParseError {
                                message: format!("Expected alias name after 'as', found {other:?}"),
                                span: self.previous_span(),
                            });
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
            return Ok(self.wrap_stmt(StmtKind::ExportList(items), start_span));
        }
        let stmt = match self.peek_kind() {
            TokenKind::KeywordFn => self.parse_fn_stmt(annotations)?,
            TokenKind::KeywordClass => self.parse_class_stmt(annotations, false)?,
            TokenKind::KeywordLet => self.parse_let_stmt()?,
            TokenKind::KeywordConst => self.parse_const_stmt()?,
            other => {
                return Err(ParseError {
                    message: format!("Expected declaration after export, found {other:?}"),
                    span: self.peek().span,
                });
            }
        };
        Ok(self.wrap_stmt(StmtKind::Export(Box::new(stmt)), start_span))
    }

    fn parse_class_stmt(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
        is_abstract: bool,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected class name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        let generics = self.parse_generic_params()?;
        let superclass = if self.matches(&TokenKind::KeywordExtends) {
            match self.advance().kind.clone() {
                TokenKind::Ident(value) => Some(value),
                other => {
                    return Err(ParseError {
                        message: format!("Expected superclass name, found {other:?}"),
                        span: self.previous_span(),
                    });
                }
            }
        } else {
            None
        };

        let mut implements = Vec::new();
        if self.matches(&TokenKind::KeywordImplements) {
            loop {
                let name = self.parse_type()?;
                implements.push(name);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }

        self.expect(TokenKind::LBrace, "Expected '{' before class body")?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut nested_classes = Vec::new();
        while !self.is_at_end() && !matches!(self.peek_kind(), TokenKind::RBrace) {
            let member_annotations = self.parse_annotations()?;
            let mut visibility = None;
            let mut is_static = false;
            let mut is_const = false;
            let mut is_abstract_member = false;
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
                    TokenKind::Hash => {
                        self.advance();
                        visibility = Some(pleno_ast::Visibility::Private);
                    }
                    TokenKind::KeywordStatic => {
                        self.advance();
                        is_static = true;
                    }
                    TokenKind::KeywordConst => {
                        self.advance();
                        is_const = true;
                    }
                    TokenKind::KeywordAbstract => {
                        self.advance();
                        is_abstract_member = true;
                    }
                    _ => break,
                }
            }
            if matches!(self.peek_kind(), TokenKind::KeywordOperator) {
                self.advance(); // consume 'operator'
                // Parse operator symbol: +, -, *, /, ==, !=, <, <=, >, >=, [], etc.
                let op_symbol = match self.peek_kind() {
                    TokenKind::Plus => {
                        self.advance();
                        "+".to_string()
                    }
                    TokenKind::Minus => {
                        self.advance();
                        "-".to_string()
                    }
                    TokenKind::Star => {
                        self.advance();
                        "*".to_string()
                    }
                    TokenKind::Slash => {
                        self.advance();
                        "/".to_string()
                    }
                    TokenKind::Percent => {
                        self.advance();
                        "%".to_string()
                    }
                    TokenKind::EqualEqual => {
                        self.advance();
                        "==".to_string()
                    }
                    TokenKind::BangEqual => {
                        self.advance();
                        "!=".to_string()
                    }
                    TokenKind::Less => {
                        self.advance();
                        "<".to_string()
                    }
                    TokenKind::LessEqual => {
                        self.advance();
                        "<=".to_string()
                    }
                    TokenKind::Greater => {
                        self.advance();
                        ">".to_string()
                    }
                    TokenKind::GreaterEqual => {
                        self.advance();
                        ">=".to_string()
                    }
                    TokenKind::LBracket => {
                        self.advance();
                        self.expect(TokenKind::RBracket, "Expected ']' after '[' in operator")?;
                        "[]".to_string()
                    }
                    other => {
                        return Err(ParseError {
                            message: format!("Expected operator symbol, found {other:?}"),
                            span: self.peek_span(),
                        });
                    }
                };
                // Parse function parameters and body
                self.expect(TokenKind::LParen, "Expected '(' after operator symbol")?;
                let params = self.parse_param_list()?;
                self.expect(TokenKind::RParen, "Expected ')' after operator parameters")?;
                let return_ty = if self.matches(&TokenKind::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let body = Box::new(self.parse_block_stmt()?);
                methods.push(pleno_ast::FunctionDecl {
                    name: format!("operator{}", op_symbol),
                    generics: Vec::new(),
                    params,
                    return_ty,
                    body,
                    is_static: false,
                    is_abstract: is_abstract_member,
                    is_async: false,
                    visibility,
                    kind: pleno_ast::FunctionKind::Operator(op_symbol),
                    annotations: member_annotations,
                });
                continue;
            }
            if matches!(self.peek_kind(), TokenKind::KeywordFn) {
                methods.push(self.parse_function_decl(FunctionDeclarationConfig {
                    kind: pleno_ast::FunctionKind::Method,
                    allow_modifiers: true,
                    visibility,
                    is_static: Some(is_static),
                    annotations: member_annotations,
                    is_async: false,
                    is_abstract: is_abstract_member,
                })?);
                continue;
            }
            if is_accessor(
                self.peek_kind(),
                self.peek_n_kind(1),
                self.peek_n_kind(2),
                "get",
            ) {
                methods.push(self.parse_accessor_decl(
                    pleno_ast::FunctionKind::Getter,
                    visibility,
                    is_static,
                    member_annotations,
                )?);
                continue;
            }
            if is_accessor(
                self.peek_kind(),
                self.peek_n_kind(1),
                self.peek_n_kind(2),
                "set",
            ) {
                methods.push(self.parse_accessor_decl(
                    pleno_ast::FunctionKind::Setter,
                    visibility,
                    is_static,
                    member_annotations,
                )?);
                continue;
            }
            if matches!(self.peek_kind(), TokenKind::KeywordProperty) {
                properties.push(self.parse_property_decl(
                    visibility,
                    is_static,
                    member_annotations,
                )?);
                continue;
            }
            if matches!(self.peek_kind(), TokenKind::KeywordClass) {
                nested_classes.push(self.parse_class_stmt(member_annotations, false)?);
                continue;
            }

            // Relaxed field declaration: visibility static? mut? identifier
            // We already handled visibility and static.
            // Check for 'mut' if not already consumed in the visibility loop
            // Actually, mut is often a keyword for fields.
            let is_mut = self.matches(&TokenKind::KeywordMut);

            if matches!(self.peek_kind(), TokenKind::Ident(name) if name == "constructor") {
                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(n) => n,
                    _ => unreachable!(),
                };
                self.expect(TokenKind::LParen, "Expected '(' after constructor")?;
                let params = self.parse_param_list()?;
                self.expect(TokenKind::RParen, "Expected ')' after parameters")?;
                let body = Box::new(self.parse_block_stmt()?);
                methods.push(pleno_ast::FunctionDecl {
                    name,
                    generics: Vec::new(),
                    params,
                    return_ty: None,
                    body,
                    is_static: false,
                    is_abstract: false,
                    is_async: false,
                    visibility: visibility.or(Some(pleno_ast::Visibility::Public)),
                    kind: pleno_ast::FunctionKind::Method,
                    annotations: member_annotations,
                });
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

            let ty = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };

            let default = if self.matches(&TokenKind::Equal) {
                Some(self.parse_expr()?)
            } else {
                None
            };

            self.expect(TokenKind::Semicolon, "Expected ';' after field declaration")?;
            fields.push(pleno_ast::FieldDecl {
                name,
                ty,
                default,
                is_static,
                is_const: is_const && !is_mut,
                visibility,
                annotations: member_annotations,
            });
        }
        self.expect(TokenKind::RBrace, "Expected '}' after class body")?;
        Ok(self.wrap_stmt(
            StmtKind::Class {
                name,
                generics,
                fields,
                methods,
                properties,
                superclass,
                implements,
                is_abstract,
                annotations,
                nested_classes,
            },
            start_span,
        ))
    }

    fn parse_property_decl(
        &mut self,
        visibility: Option<pleno_ast::Visibility>,
        is_static: bool,
        annotations: Vec<pleno_ast::Annotation>,
    ) -> Result<pleno_ast::PropertyDecl, ParseError> {
        self.advance(); // consume 'property'
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            TokenKind::KeywordGet => "get".to_string(),
            TokenKind::KeywordSet => "set".to_string(),
            other => {
                return Err(ParseError {
                    message: format!("Expected property name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };

        let ty = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(TokenKind::LBrace, "Expected '{' after property name")?;

        let mut getter = None;
        let mut setter = None;

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.matches(&TokenKind::KeywordGet) {
                if getter.is_some() {
                    return Err(ParseError {
                        message: "Property already has a getter".to_string(),
                        span: self.previous_span(),
                    });
                }
                let body = Box::new(self.parse_block_stmt()?);
                getter = Some(pleno_ast::FunctionDecl {
                    name: format!("get_{}", name),
                    generics: Vec::new(),
                    params: Vec::new(),
                    return_ty: ty.clone(),
                    body,
                    is_static,
                    is_abstract: false,
                    is_async: false,
                    visibility: visibility.clone(),
                    kind: pleno_ast::FunctionKind::Getter,
                    annotations: Vec::new(),
                });
            } else if self.matches(&TokenKind::KeywordSet) {
                if setter.is_some() {
                    return Err(ParseError {
                        message: "Property already has a setter".to_string(),
                        span: self.previous_span(),
                    });
                }
                self.expect(TokenKind::LParen, "Expected '(' after 'set'")?;
                let param_name = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(ParseError {
                            message: format!("Expected parameter name in setter, found {other:?}"),
                            span: self.previous_span(),
                        });
                    }
                };
                self.expect(TokenKind::RParen, "Expected ')' after parameter name")?;
                let body = Box::new(self.parse_block_stmt()?);
                setter = Some(pleno_ast::FunctionDecl {
                    name: format!("set_{}", name),
                    generics: Vec::new(),
                    params: vec![pleno_ast::Param {
                        name: param_name,
                        ty: ty.clone(),
                        default: None,
                        is_variadic: false,
                        is_keyword_only: false,
                    }],
                    return_ty: Some("Void".to_string()),
                    body,
                    is_static,
                    is_abstract: false,
                    is_async: false,
                    visibility: visibility.clone(),
                    kind: pleno_ast::FunctionKind::Setter,
                    annotations: Vec::new(),
                });
            } else {
                return Err(ParseError {
                    message: format!(
                        "Expected 'get' or 'set' in property body, found {:?}",
                        self.peek_kind()
                    ),
                    span: self.peek_span(),
                });
            }
        }

        self.expect(TokenKind::RBrace, "Expected '}' after property body")?;

        Ok(pleno_ast::PropertyDecl {
            name,
            ty,
            getter,
            setter,
            is_static,
            visibility,
            annotations,
        })
    }

    fn parse_generic_params(&mut self) -> Result<Vec<pleno_ast::GenericParam>, ParseError> {
        if !self.matches(&TokenKind::Less) {
            return Ok(Vec::new());
        }
        let mut params = Vec::new();
        loop {
            let name = match self.advance().kind.clone() {
                TokenKind::Ident(name) => name,
                other => {
                    return Err(ParseError {
                        message: format!("Expected generic parameter name, found {other:?}"),
                        span: self.previous_span(),
                    });
                }
            };
            let mut constraints = Vec::new();
            if self.matches(&TokenKind::Colon) {
                // Parse constraints
                loop {
                    let constraint = match self.advance().kind.clone() {
                        TokenKind::Ident(name) => name,
                        other => {
                            return Err(ParseError {
                                message: format!("Expected constraint name, found {other:?}"),
                                span: self.previous_span(),
                            });
                        }
                    };
                    constraints.push(constraint);
                    // Support both + and & for constraints (specs use &, but + is common)
                    if !self.matches(&TokenKind::Plus) && !self.matches(&TokenKind::Ampersand) {
                        break;
                    }
                }
            }
            params.push(pleno_ast::GenericParam { name, constraints });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::Greater, "Expected '>' after generic parameters")?;
        Ok(params)
    }

    fn parse_function_decl(
        &mut self,
        config: FunctionDeclarationConfig,
    ) -> Result<pleno_ast::FunctionDecl, ParseError> {
        self.advance();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(name) => name,
            TokenKind::KeywordMatch => "match".to_string(),
            TokenKind::KeywordGet => "get".to_string(),
            TokenKind::KeywordSet => "set".to_string(),
            other => {
                return Err(ParseError {
                    message: format!("Expected function name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        let generics = self.parse_generic_params()?;
        self.expect(TokenKind::LParen, "Expected '(' after function name")?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen, "Expected ')' after parameters")?;
        let return_ty = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = if config.is_abstract {
            self.expect(TokenKind::Semicolon, "Expected ';' after abstract function")?;
            Box::new(self.wrap_stmt(StmtKind::Block(Vec::new()), self.previous_span()))
        } else {
            Box::new(self.parse_block_stmt()?)
        };

        let (visibility, is_static) = if config.allow_modifiers {
            (config.visibility, config.is_static.unwrap_or(false))
        } else {
            if config.visibility.is_some() || config.is_static.unwrap_or(false) {
                return Err(ParseError {
                    message: "Function modifiers are only allowed in classes".to_string(),
                    span: self.previous_span(),
                });
            }
            (None, false)
        };
        Ok(pleno_ast::FunctionDecl {
            name,
            generics,
            params,
            return_ty,
            body,
            is_static,
            is_abstract: config.is_abstract,
            is_async: config.is_async,
            visibility,
            kind: config.kind,
            annotations: config.annotations,
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
            generics: Vec::new(),
            params,
            return_ty: None,
            body: Box::new(body),
            is_static,
            is_abstract: false,
            is_async: false,
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
                    });
                }
            };
            let ty = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
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
                ty,
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

    fn parse_lambda_params_in_parens(&mut self) -> Result<Vec<pleno_ast::Param>, ParseError> {
        let mut params = Vec::new();
        if self.matches(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let name = match self.advance().kind.clone() {
                TokenKind::Ident(name) => name,
                TokenKind::Underscore => "_".to_string(),
                other => {
                    return Err(ParseError {
                        message: format!("Expected parameter name in lambda, found {other:?}"),
                        span: self.previous_span(),
                    });
                }
            };
            params.push(pleno_ast::Param {
                name,
                ty: None,
                default: None,
                is_variadic: false,
                is_keyword_only: false,
            });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
            if matches!(self.peek_kind(), TokenKind::RParen) {
                break;
            }
        }
        self.expect(TokenKind::RParen, "Expected ')' after lambda parameters")?;
        Ok(params)
    }

    fn parse_block_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.expect(TokenKind::LBrace, "Expected '{' to start block")?;
        self.parse_braced_block_content(start_span)
    }

    fn parse_braced_block_content(&mut self, start_span: Span) -> Result<Stmt, ParseError> {
        let mut statements = Vec::new();
        while !self.is_at_end() && self.peek_kind() != &TokenKind::RBrace {
            statements.push(self.parse_stmt()?);
        }
        self.expect(TokenKind::RBrace, "Expected '}' after block")?;
        Ok(self.wrap_stmt(StmtKind::Block(statements), start_span))
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_pipeline()?;
        let start_span = expr.span;

        if self.matches(&TokenKind::Question) {
            let then_branch = self.parse_expr()?;
            self.expect(TokenKind::Colon, "Expected ':' in ternary operator")?;
            let else_branch = self.parse_expr()?;
            return Ok(self.wrap_expr(
                ExprKind::If {
                    condition: Box::new(expr),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                },
                start_span,
            ));
        }

        Ok(expr)
    }

    fn parse_pipeline(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_or()?;
        let start_span = expr.span;
        while self.matches(&TokenKind::Pipeline) {
            let right = self.parse_or()?;
            expr = self.wrap_expr(
                ExprKind::Pipeline {
                    left: Box::new(expr),
                    right: Box::new(right),
                },
                start_span,
            );
        }
        Ok(expr)
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_null_coalesce()?;
        let start_span = expr.span;
        while self.matches(&TokenKind::OrOr) {
            let right = self.parse_null_coalesce()?;
            expr = self.wrap_expr(
                ExprKind::Binary {
                    left: Box::new(expr),
                    op: pleno_ast::BinaryOp::Or,
                    right: Box::new(right),
                },
                start_span,
            );
        }
        Ok(expr)
    }

    fn parse_null_coalesce(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_and()?;
        let start_span = expr.span;
        while self.matches(&TokenKind::NullCoalesce) {
            let right = self.parse_and()?;
            expr = self.wrap_expr(
                ExprKind::Binary {
                    left: Box::new(expr),
                    op: pleno_ast::BinaryOp::NullCoalesce,
                    right: Box::new(right),
                },
                start_span,
            );
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_equality()?;
        let start_span = expr.span;
        while self.matches(&TokenKind::AndAnd) {
            let right = self.parse_equality()?;
            expr = self.wrap_expr(
                ExprKind::Binary {
                    left: Box::new(expr),
                    op: pleno_ast::BinaryOp::And,
                    right: Box::new(right),
                },
                start_span,
            );
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_is_expr()?;
        while self.matches(&TokenKind::EqualEqual) || self.matches(&TokenKind::BangEqual) {
            let op = match self.previous().kind {
                TokenKind::EqualEqual => pleno_ast::BinaryOp::Equal,
                TokenKind::BangEqual => pleno_ast::BinaryOp::NotEqual,
                _ => unreachable!(),
            };
            let right = self.parse_is_expr()?;
            expr = self.wrap_expr(
                ExprKind::Binary {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                },
                self.previous_span(),
            );
        }
        Ok(expr)
    }

    fn parse_is_expr(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_comparison()?;
        if self.matches(&TokenKind::KeywordIs) {
            let ty = self.parse_type()?;
            expr = self.wrap_expr(
                ExprKind::Is {
                    left: Box::new(expr),
                    ty,
                },
                self.previous_span(),
            );
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_range()?;
        let start_span = expr.span;
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
            expr = self.wrap_expr(
                ExprKind::Binary {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                },
                start_span,
            );
        }
        Ok(expr)
    }

    fn parse_range(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.peek_span();
        if self.matches(&TokenKind::DotDot) {
            let right = if self.is_range_terminator() {
                None
            } else {
                Some(Box::new(self.parse_term()?))
            };
            return Ok(self.wrap_expr(
                ExprKind::Range {
                    start: None,
                    end: right,
                    inclusive: false,
                },
                start_span,
            ));
        }
        if self.matches(&TokenKind::DotDotEqual) {
            let right = if self.is_range_terminator() {
                None
            } else {
                Some(Box::new(self.parse_term()?))
            };
            return Ok(self.wrap_expr(
                ExprKind::Range {
                    start: None,
                    end: right,
                    inclusive: true,
                },
                start_span,
            ));
        }
        let left = self.parse_term()?;
        let start_span = left.span;
        if self.matches(&TokenKind::DotDot) {
            let right = if self.is_range_terminator() {
                None
            } else {
                Some(Box::new(self.parse_term()?))
            };
            return Ok(self.wrap_expr(
                ExprKind::Range {
                    start: Some(Box::new(left)),
                    end: right,
                    inclusive: false,
                },
                start_span,
            ));
        }
        if self.matches(&TokenKind::DotDotEqual) {
            let right = if self.is_range_terminator() {
                None
            } else {
                Some(Box::new(self.parse_term()?))
            };
            return Ok(self.wrap_expr(
                ExprKind::Range {
                    start: Some(Box::new(left)),
                    end: right,
                    inclusive: true,
                },
                start_span,
            ));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_factor()?;
        let start_span = expr.span;
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
            expr = self.wrap_expr(
                ExprKind::Binary {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                },
                start_span,
            );
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_unary()?;
        let start_span = expr.span;
        loop {
            let op = if self.matches(&TokenKind::Star) {
                Some(pleno_ast::BinaryOp::Multiply)
            } else if self.matches(&TokenKind::Slash) {
                Some(pleno_ast::BinaryOp::Divide)
            } else if self.matches(&TokenKind::Percent) {
                Some(pleno_ast::BinaryOp::Modulo)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_unary()?;
            expr = self.wrap_expr(
                ExprKind::Binary {
                    left: Box::new(expr),
                    op,
                    right: Box::new(right),
                },
                start_span,
            );
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.peek_span();
        if self.matches(&TokenKind::Bang) {
            let expr = self.parse_unary()?;
            return Ok(self.wrap_expr(
                ExprKind::Unary {
                    op: pleno_ast::UnaryOp::Not,
                    expr: Box::new(expr),
                },
                start_span,
            ));
        }
        if self.matches(&TokenKind::Minus) {
            let expr = self.parse_unary()?;
            return Ok(self.wrap_expr(
                ExprKind::Unary {
                    op: pleno_ast::UnaryOp::Negate,
                    expr: Box::new(expr),
                },
                start_span,
            ));
        }
        self.parse_primary()
    }

    fn parse_lambda_params(&mut self) -> Result<Vec<pleno_ast::Param>, ParseError> {
        let mut params = Vec::new();
        if matches!(self.peek_kind(), TokenKind::Pipe) {
            return Ok(params);
        }
        loop {
            let name = match self.advance().kind.clone() {
                TokenKind::Ident(name) => name,
                TokenKind::Underscore => "_".to_string(),
                other => {
                    return Err(ParseError {
                        message: format!("Expected parameter name in lambda, found {other:?}"),
                        span: self.previous_span(),
                    });
                }
            };
            params.push(pleno_ast::Param {
                name,
                ty: None,
                default: None,
                is_variadic: false,
                is_keyword_only: false,
            });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_atom()?;
        self.parse_postfix_loop(expr, true)
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance();
        let start_span = token.span;
        match token.kind.clone() {
            TokenKind::Int(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::Int(value)), start_span))
            }
            TokenKind::Float(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::Float(value)), start_span))
            }
            TokenKind::String(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::String(value)), start_span))
            }
            TokenKind::Char(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::Char(value)), start_span))
            }
            TokenKind::Bool(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::Bool(value)), start_span))
            }
            TokenKind::Null => Ok(self.wrap_expr(ExprKind::Literal(Literal::Null), start_span)),
            TokenKind::Ident(name) => Ok(self.wrap_expr(ExprKind::Ident(name), start_span)),
            TokenKind::Underscore => Ok(self.wrap_expr(ExprKind::Underscore, start_span)),
            TokenKind::KeywordMatch => self.parse_match_expr(start_span),
            TokenKind::KeywordNew => {
                let atom = self.parse_atom()?;
                let callee = self.parse_postfix_loop(atom, false)?;

                let args = if self.matches(&TokenKind::LParen) {
                    let mut args = Vec::new();
                    if !matches!(self.peek_kind(), TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.matches(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(
                        TokenKind::RParen,
                        "Expected ')' after constructor arguments",
                    )?;
                    args
                } else {
                    Vec::new()
                };

                Ok(self.wrap_expr(
                    ExprKind::Call {
                        callee: Box::new(callee),
                        args: args
                            .into_iter()
                            .map(pleno_ast::CallArg::Positional)
                            .collect(),
                    },
                    start_span,
                ))
            }
            TokenKind::KeywordThis => Ok(self.wrap_expr(ExprKind::This, start_span)),
            TokenKind::KeywordSuper => Ok(self.wrap_expr(ExprKind::Super, start_span)),
            TokenKind::KeywordAwait => {
                let expr = Box::new(self.parse_unary()?);
                Ok(self.wrap_expr(ExprKind::Await { expr }, start_span))
            }
            TokenKind::Pipe => self.parse_lambda_expr(Vec::new(), start_span),
            TokenKind::KeywordIf => self.parse_if_expr(start_span),
            TokenKind::KeywordFn => self.parse_fn_expr(Vec::new(), start_span),
            TokenKind::At => {
                let first_annotation = self.parse_annotation()?;
                let mut annotations = vec![first_annotation];
                annotations.extend(self.parse_annotations()?);
                let start_token = self.advance();
                match start_token.kind {
                    TokenKind::Pipe => self.parse_lambda_expr(annotations, start_span),
                    TokenKind::KeywordFn => self.parse_fn_expr(annotations, start_span),
                    _ => Err(ParseError {
                        message: "Annotations only allowed before lambda functions in expressions"
                            .to_string(),
                        span: start_span,
                    }),
                }
            }
            TokenKind::LBrace => {
                if matches!(self.peek_kind(), TokenKind::RBrace) {
                    self.expect(TokenKind::RBrace, "Expected '}' after map literal")?;
                    Ok(self.wrap_expr(ExprKind::MapLiteral(Vec::new()), start_span))
                } else {
                    match self.lookahead_braced_kind()? {
                        BracedKind::Set => self.parse_set_literal(start_span),
                        BracedKind::Map => self.parse_map_literal(start_span),
                        BracedKind::Block => {
                            let block = self.parse_braced_block_content(start_span)?;
                            if let StmtKind::Block(stmts) = block.kind {
                                Ok(self.wrap_expr(ExprKind::Block(stmts), block.span))
                            } else {
                                unreachable!()
                            }
                        }
                    }
                }
            }
            TokenKind::LBracket => {
                let mut items = Vec::new();
                if !matches!(self.peek_kind(), TokenKind::RBracket) {
                    loop {
                        let inner_start_span = self.peek_span();
                        let value = if self.matches(&TokenKind::DotDot)
                            || self.matches(&TokenKind::Ellipsis)
                        {
                            let inner_expr = self.parse_expr()?;
                            self.wrap_expr(ExprKind::Spread(Box::new(inner_expr)), inner_start_span)
                        } else {
                            self.parse_expr()?
                        };
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
                Ok(self.wrap_expr(ExprKind::ArrayLiteral(items), start_span))
            }
            TokenKind::LParen => {
                if self.lookahead_is_arrow_lambda() {
                    let params = self.parse_lambda_params_in_parens()?;
                    self.expect(TokenKind::FatArrow, "Expected '=>' after lambda parameters")?;
                    let body = if matches!(self.peek_kind(), TokenKind::LBrace) {
                        let block = self.parse_block_stmt()?;
                        if let StmtKind::Block(stmts) = block.kind {
                            self.wrap_expr(ExprKind::Block(stmts), block.span)
                        } else {
                            unreachable!()
                        }
                    } else {
                        self.parse_expr()?
                    };
                    Ok(self.wrap_expr(
                        ExprKind::Lambda {
                            params,
                            body: Box::new(body),
                            return_ty: None,
                            annotations: Vec::new(),
                        },
                        start_span,
                    ))
                } else if matches!(self.peek_kind(), TokenKind::RParen) {
                    self.advance();
                    Ok(self.wrap_expr(ExprKind::TupleLiteral(Vec::new()), start_span))
                } else if matches!(self.peek_kind(), TokenKind::Ident(_))
                    && matches!(self.peek_next_kind(), Some(TokenKind::Colon))
                {
                    let mut entries = Vec::new();
                    loop {
                        let name = match self.advance().kind.clone() {
                            TokenKind::Ident(name) => name,
                            _ => unreachable!(),
                        };
                        self.expect(TokenKind::Colon, "Expected ':' after named tuple field")?;
                        let value = self.parse_expr()?;
                        entries.push((name, value));
                        if !self.matches(&TokenKind::Comma) {
                            break;
                        }
                        if matches!(self.peek_kind(), TokenKind::RParen) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen, "Expected ')' after named tuple")?;
                    Ok(self.wrap_expr(ExprKind::NamedTupleLiteral(entries), start_span))
                } else {
                    let first = self.parse_expr()?;
                    if self.matches(&TokenKind::Comma) {
                        let mut items = vec![first];
                        while !matches!(self.peek_kind(), TokenKind::RParen) {
                            items.push(self.parse_expr()?);
                            if !self.matches(&TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(TokenKind::RParen, "Expected ')' after tuple")?;
                        Ok(self.wrap_expr(ExprKind::TupleLiteral(items), start_span))
                    } else {
                        self.expect(TokenKind::RParen, "Expected ')' after expression")?;
                        Ok(self.wrap_expr(ExprKind::Paren(Box::new(first)), start_span))
                    }
                }
            }
            TokenKind::Dollar => {
                let dollar_span = token.span;
                if !self.matches(&TokenKind::LBrace) {
                    return Err(ParseError {
                        message: "Expected '{' after '$' for macro splice".to_string(),
                        span: dollar_span,
                    });
                }
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RBrace, "Expected '}' after macro splice")?;
                Ok(self.wrap_expr(ExprKind::MacroSplice(Box::new(expr)), start_span))
            }
            other => Err(ParseError {
                message: format!("Unexpected token in expression: {other:?}"),
                span: token.span,
            }),
        }
    }

    fn parse_postfix_loop(&mut self, mut expr: Expr, allow_call: bool) -> Result<Expr, ParseError> {
        loop {
            let start_span = expr.span;
            if self.matches(&TokenKind::Dot) {
                // Support .#name for private access
                if self.matches(&TokenKind::Hash) {
                    let name = match self.advance().kind.clone() {
                        TokenKind::Ident(name) => name,
                        other => {
                            return Err(ParseError {
                                message: format!("Expected identifier after '.#', found {other:?}"),
                                span: self.previous_span(),
                            });
                        }
                    };
                    expr = self.wrap_expr(
                        ExprKind::Property {
                            object: Box::new(expr),
                            name,
                        },
                        start_span,
                    );
                    continue;
                }

                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(name) => name,
                    TokenKind::KeywordMatch => "match".to_string(),
                    TokenKind::KeywordGet => "get".to_string(),
                    TokenKind::KeywordSet => "set".to_string(),
                    TokenKind::Int(val) => val.to_string(),
                    other => {
                        return Err(ParseError {
                            message: format!(
                                "Expected identifier or index after '.', found {other:?}"
                            ),
                            span: self.previous_span(),
                        });
                    }
                };
                expr = self.wrap_expr(
                    ExprKind::Property {
                        object: Box::new(expr),
                        name,
                    },
                    start_span,
                );
                continue;
            }
            if self.matches(&TokenKind::QuestionDot) {
                if self.matches(&TokenKind::LParen) {
                    let args = self.parse_call_args()?;
                    expr = self.wrap_expr(
                        ExprKind::SafeCall {
                            callee: Box::new(expr),
                            args,
                        },
                        start_span,
                    );
                    continue;
                }
                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(name) => name,
                    TokenKind::Int(val) => val.to_string(),
                    other => {
                        return Err(ParseError {
                            message: format!(
                                "Expected identifier or index after '?.', found {other:?}"
                            ),
                            span: self.previous_span(),
                        });
                    }
                };
                expr = self.wrap_expr(
                    ExprKind::SafeProperty {
                        object: Box::new(expr),
                        name,
                    },
                    start_span,
                );
                continue;
            }
            if self.matches(&TokenKind::LBracket) {
                // Check if this is a slice starting with separator: [:end] or [..end]
                if self.matches(&TokenKind::DotDot)
                    || self.matches(&TokenKind::DotDotEqual)
                    || self.matches(&TokenKind::Colon)
                {
                    // Slice: [:end], [..end] or [..=end]
                    let _inclusive = self.previous().kind == TokenKind::DotDotEqual;
                    let end = if self.matches(&TokenKind::RBracket) || self.is_range_terminator() {
                        None
                    } else {
                        let expr = Box::new(self.parse_expr()?);
                        self.expect(TokenKind::RBracket, "Expected ']' after slice")?;
                        Some(expr)
                    };
                    expr = self.wrap_expr(
                        ExprKind::Slice {
                            collection: Box::new(expr),
                            start: None,
                            end,
                        },
                        start_span,
                    );
                    continue;
                }

                let start_idx = self.parse_expr()?;

                // Check for slice separator: [start:end] or [start..end]
                if self.matches(&TokenKind::DotDot)
                    || self.matches(&TokenKind::DotDotEqual)
                    || self.matches(&TokenKind::Colon)
                {
                    let _inclusive = self.previous().kind == TokenKind::DotDotEqual;
                    let end = if self.matches(&TokenKind::RBracket) || self.is_range_terminator() {
                        None
                    } else {
                        let expr = Box::new(self.parse_expr()?);
                        self.expect(TokenKind::RBracket, "Expected ']' after slice")?;
                        Some(expr)
                    };
                    expr = self.wrap_expr(
                        ExprKind::Slice {
                            collection: Box::new(expr),
                            start: Some(Box::new(start_idx)),
                            end,
                        },
                        start_span,
                    );
                    continue;
                }

                // Regular index access
                self.expect(TokenKind::RBracket, "Expected ']' after index")?;
                expr = self.wrap_expr(
                    ExprKind::Index {
                        collection: Box::new(expr),
                        index: Box::new(start_idx),
                    },
                    start_span,
                );
                continue;
            }
            if self.matches(&TokenKind::Less) {
                let mut type_args = Vec::new();
                loop {
                    type_args.push(self.parse_type()?);
                    if self.matches(&TokenKind::Comma) {
                        continue;
                    }
                    break;
                }
                self.expect(
                    TokenKind::Greater,
                    "Expected '>' after generic type arguments",
                )?;
                expr = self.wrap_expr(
                    ExprKind::Specialize {
                        left: Box::new(expr),
                        type_args,
                    },
                    start_span,
                );
                continue;
            }
            if allow_call && self.matches(&TokenKind::LParen) {
                let args = self.parse_call_args()?;
                expr = self.wrap_expr(
                    ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    start_span,
                );
                continue;
            }

            if self.matches(&TokenKind::Bang) {
                if allow_call
                    && matches!(expr.kind, ExprKind::Ident(_))
                    && self.check(&TokenKind::LParen)
                {
                    let name = if let ExprKind::Ident(name) = expr.kind {
                        name
                    } else {
                        unreachable!()
                    };
                    self.advance(); // consume '('
                    let args = self.parse_call_args()?;
                    expr = self.wrap_expr(ExprKind::MacroCall { name, args }, start_span);
                    continue;
                }
                expr = self.wrap_expr(ExprKind::AssertNotNull(Box::new(expr)), start_span);
                continue;
            }

            break;
        }
        Ok(expr)
    }

    fn parse_call_args(&mut self) -> Result<Vec<pleno_ast::CallArg>, ParseError> {
        let mut args = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RParen) {
            loop {
                let arg = if matches!(self.peek_kind(), TokenKind::Underscore) {
                    self.advance();
                    pleno_ast::CallArg::Placeholder
                } else if matches!(self.peek_kind(), TokenKind::Ident(_))
                    && matches!(self.peek_next_kind(), Some(TokenKind::Colon))
                {
                    let name = match self.advance().kind.clone() {
                        TokenKind::Ident(value) => value,
                        _ => unreachable!("ident checked"),
                    };
                    self.expect(TokenKind::Colon, "Expected ':' in named argument")?;
                    let value = self.parse_expr()?;
                    pleno_ast::CallArg::Named(name, value)
                } else if self.matches(&TokenKind::Ellipsis) || self.matches(&TokenKind::DotDot) {
                    pleno_ast::CallArg::Spread(self.parse_expr()?)
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
        Ok(args)
    }

    fn parse_map_literal(&mut self, start_span: Span) -> Result<Expr, ParseError> {
        let mut entries = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RBrace) {
            loop {
                if self.matches(&TokenKind::DotDot) || self.matches(&TokenKind::Ellipsis) {
                    let value = self.parse_expr()?;
                    entries.push(pleno_ast::MapItem::Spread(value));
                } else {
                    let key = match self.advance().kind.clone() {
                        TokenKind::String(value) => value,
                        other => {
                            return Err(ParseError {
                                message: format!(
                                    "Expected string literal key in map, found {other:?}"
                                ),
                                span: self.previous_span(),
                            });
                        }
                    };
                    self.expect(TokenKind::Colon, "Expected ':' after map key")?;
                    let value = self.parse_expr()?;
                    entries.push(pleno_ast::MapItem::Entry(key, value));
                }
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
        Ok(self.wrap_expr(ExprKind::MapLiteral(entries), start_span))
    }

    fn parse_set_literal(&mut self, start_span: Span) -> Result<Expr, ParseError> {
        let mut entries = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RBrace) {
            loop {
                let inner_start_span = self.peek_span();
                let value =
                    if self.matches(&TokenKind::DotDot) || self.matches(&TokenKind::Ellipsis) {
                        let inner_expr = self.parse_expr()?;
                        self.wrap_expr(ExprKind::Spread(Box::new(inner_expr)), inner_start_span)
                    } else {
                        self.parse_expr()?
                    };
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
        Ok(self.wrap_expr(ExprKind::SetLiteral(entries), start_span))
    }

    fn lookahead_braced_kind(&mut self) -> Result<BracedKind, ParseError> {
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
                TokenKind::Colon if depth == 1 => return Ok(BracedKind::Map),
                TokenKind::Semicolon if depth == 1 => return Ok(BracedKind::Block),
                TokenKind::KeywordReturn if depth == 1 => return Ok(BracedKind::Block),
                TokenKind::KeywordLet if depth == 1 => return Ok(BracedKind::Block),
                TokenKind::KeywordIf if depth == 1 => return Ok(BracedKind::Block),
                TokenKind::KeywordFor if depth == 1 => return Ok(BracedKind::Block),
                TokenKind::KeywordWhile if depth == 1 => return Ok(BracedKind::Block),
                _ => {}
            }
            i += 1;
        }

        // If it looks like { x } it's ambiguous but we prefer Set in expression position
        // unless it's obviously a block.
        Ok(BracedKind::Set)
    }

    fn parse_match_expr(&mut self, start_span: Span) -> Result<Expr, ParseError> {
        let expr = Box::new(self.parse_expr()?);
        self.expect(TokenKind::LBrace, "Expected '{' after match expression")?;
        let mut arms = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace) {
            self.expect(TokenKind::KeywordCase, "Expected 'case' before pattern")?;

            // Multiple patterns separated by comma
            let mut patterns = Vec::new();
            loop {
                patterns.push(self.parse_pattern()?);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
                // Check if next is FatArrow or if, in which case the comma was trailing
                if matches!(self.peek_kind(), TokenKind::FatArrow | TokenKind::KeywordIf) {
                    break;
                }
            }

            let pattern = if patterns.len() > 1 {
                pleno_ast::Pattern::Or(patterns)
            } else {
                patterns.remove(0)
            };

            // Optional guard: if <expr>
            let guard = if self.matches(&TokenKind::KeywordIf) {
                Some(self.parse_expr()?)
            } else {
                None
            };

            self.expect(TokenKind::FatArrow, "Expected '=>' after pattern")?;
            let body = self.parse_expr()?;
            arms.push(pleno_ast::MatchArm {
                pattern,
                guard,
                body,
            });

            if !self.matches(&TokenKind::Comma) {
                // Comma between arms is optional but encouraged
            }
        }
        self.expect(TokenKind::RBrace, "Expected '}' after match arms")?;
        Ok(self.wrap_expr(ExprKind::Match { expr, arms }, start_span))
    }

    fn parse_pattern(&mut self) -> Result<pleno_ast::Pattern, ParseError> {
        let token = self.advance();
        match token.kind.clone() {
            TokenKind::Underscore => Ok(pleno_ast::Pattern::Underscore),
            TokenKind::Ident(mut name) => {
                let mut qualified = false;
                while self.matches(&TokenKind::Dot) {
                    qualified = true;
                    if let TokenKind::Ident(subname) = self.advance().kind.clone() {
                        name = subname;
                    } else {
                        return Err(ParseError {
                            message: "Expected variant name after '.'".to_string(),
                            span: self.previous_span(),
                        });
                    }
                }

                // Variant: name(...)
                if self.matches(&TokenKind::LParen) {
                    let mut patterns = Vec::new();
                    if !matches!(self.peek_kind(), TokenKind::RParen) {
                        loop {
                            patterns.push(self.parse_pattern()?);
                            if !self.matches(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen, "Expected ')' after variant pattern")?;
                    return Ok(pleno_ast::Pattern::Variant(name, patterns));
                }

                // If it was qualified, it's a variant pattern even without parameters
                if qualified {
                    return Ok(pleno_ast::Pattern::Variant(name, Vec::new()));
                }

                // Type pattern: Type ident or Type _
                if matches!(
                    self.peek_kind(),
                    TokenKind::Ident(_) | TokenKind::Underscore
                ) {
                    let binding = match &self.advance().kind {
                        TokenKind::Ident(b) => Some(b.clone()),
                        TokenKind::Underscore => None,
                        _ => unreachable!(),
                    };
                    return Ok(pleno_ast::Pattern::Type(name, binding));
                }

                Ok(pleno_ast::Pattern::Ident(name))
            }
            TokenKind::LBrace => {
                let mut patterns = Vec::new();
                let mut rest = None;
                if !matches!(self.peek_kind(), TokenKind::RBrace) {
                    loop {
                        if matches!(self.peek_kind(), TokenKind::DotDot | TokenKind::Ellipsis) {
                            self.advance();
                            let name = if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                                match &self.advance().kind {
                                    TokenKind::Ident(name) => Some(name.clone()),
                                    _ => unreachable!(),
                                }
                            } else {
                                None
                            };
                            rest = Some(Box::new(pleno_ast::Pattern::Rest(name)));
                            if self.matches(&TokenKind::Comma) {
                                if !matches!(self.peek_kind(), TokenKind::RBrace) {
                                    return Err(ParseError {
                                        message: "Rest pattern must be the last element"
                                            .to_string(),
                                        span: self.previous_span(),
                                    });
                                }
                            }
                            break;
                        }

                        let field_name = match &self.advance().kind {
                            TokenKind::Ident(name) => name.clone(),
                            other => {
                                return Err(ParseError {
                                    message: format!(
                                        "Expected field name in object pattern, found {other:?}"
                                    ),
                                    span: self.previous_span(),
                                });
                            }
                        };

                        let pattern = if self.matches(&TokenKind::Colon) {
                            self.parse_pattern()?
                        } else {
                            // Shorthand: { x } is { x: x }
                            pleno_ast::Pattern::Ident(field_name.clone())
                        };

                        patterns.push((field_name, pattern));

                        if !self.matches(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RBrace, "Expected '}' after object pattern")?;
                Ok(pleno_ast::Pattern::Object(patterns, rest))
            }
            TokenKind::LBracket => {
                let mut items = Vec::new();
                if !matches!(self.peek_kind(), TokenKind::RBracket) {
                    loop {
                        items.push(self.parse_pattern()?);
                        if !self.matches(&TokenKind::Comma) {
                            break;
                        }
                        if matches!(self.peek_kind(), TokenKind::RBracket) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RBracket, "Expected ']' after array pattern")?;
                Ok(pleno_ast::Pattern::Tuple(items))
            }
            TokenKind::LParen => {
                let mut items = Vec::new();
                if !matches!(self.peek_kind(), TokenKind::RParen) {
                    loop {
                        items.push(self.parse_pattern()?);
                        if !self.matches(&TokenKind::Comma) {
                            break;
                        }
                        if matches!(self.peek_kind(), TokenKind::RParen) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RParen, "Expected ')' after tuple pattern")?;
                Ok(pleno_ast::Pattern::Tuple(items))
            }
            TokenKind::Int(val) => Ok(pleno_ast::Pattern::Literal(pleno_ast::Literal::Int(val))),
            TokenKind::Float(val) => {
                Ok(pleno_ast::Pattern::Literal(pleno_ast::Literal::Float(val)))
            }
            TokenKind::String(val) => {
                Ok(pleno_ast::Pattern::Literal(pleno_ast::Literal::String(val)))
            }
            TokenKind::Bool(val) => Ok(pleno_ast::Pattern::Literal(pleno_ast::Literal::Bool(val))),
            TokenKind::Null => Ok(pleno_ast::Pattern::Literal(pleno_ast::Literal::Null)),
            TokenKind::DotDot | TokenKind::Ellipsis => {
                let name = if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                    match &self.advance().kind {
                        TokenKind::Ident(name) => Some(name.clone()),
                        _ => unreachable!(),
                    }
                } else {
                    None
                };
                Ok(pleno_ast::Pattern::Rest(name))
            }
            other => Err(ParseError {
                message: format!("Expected pattern, found {other:?}"),
                span: self.previous_span(),
            }),
        }
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

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == kind
    }

    fn peek_next_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.index + 1).map(|token| &token.kind)
    }

    fn peek_n_kind(&self, offset: usize) -> Option<&TokenKind> {
        self.tokens
            .get(self.index + offset)
            .map(|token| &token.kind)
    }

    fn lookahead_is_arrow_lambda(&self) -> bool {
        let mut depth = 1;
        let mut idx = self.index;
        while let Some(token) = self.tokens.get(idx) {
            match &token.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(idx + 1).map(|token| &token.kind),
                            Some(TokenKind::FatArrow)
                        );
                    }
                }
                _ => {}
            }
            idx += 1;
        }
        false
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

    fn parse_type_alias_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // type
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(ParseError {
                    message: format!("Expected type name after 'type', found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };
        self.expect(TokenKind::Equal, "Expected '=' after type alias name")?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Semicolon, "Expected ';' after type alias")?;
        Ok(self.wrap_stmt(StmtKind::TypeAlias { name, ty }, start_span))
    }

    fn parse_type(&mut self) -> Result<String, ParseError> {
        self.parse_union_type()
    }

    fn parse_union_type(&mut self) -> Result<String, ParseError> {
        let mut types = vec![self.parse_intersection_type()?];
        while self.matches(&TokenKind::Pipe) {
            types.push(self.parse_intersection_type()?);
        }
        if types.len() == 1 {
            Ok(types.into_iter().next().unwrap())
        } else {
            Ok(types.join(" | "))
        }
    }

    fn parse_intersection_type(&mut self) -> Result<String, ParseError> {
        let mut types = vec![self.parse_base_type()?];
        while self.matches(&TokenKind::Ampersand) {
            types.push(self.parse_base_type()?);
        }
        if types.len() == 1 {
            Ok(types.into_iter().next().unwrap())
        } else {
            Ok(types.join(" & "))
        }
    }

    fn parse_base_type(&mut self) -> Result<String, ParseError> {
        // Ownership hints: move T
        if self.matches(&TokenKind::KeywordMove) {
            let inner = self.parse_base_type()?;
            return Ok(format!("move {}", inner));
        }

        // Reference types: &T or &mut T
        if self.check(&TokenKind::Ampersand) {
            let start_span = self.peek_span();
            self.advance(); // consume '&'
            let mut type_str = String::from("&");
            if self.matches(&TokenKind::KeywordMut) {
                type_str.push_str("mut ");
            }

            // Base type name after & / &mut
            let base_name = match self.advance().kind.clone() {
                TokenKind::Ident(name) => name,
                TokenKind::Null => "Null".to_string(),
                other => {
                    return Err(ParseError {
                        message: format!("Expected type name after '&', found {other:?}"),
                        span: start_span,
                    });
                }
            };
            type_str.push_str(&base_name);

            // Generic arguments: &Option<Int>
            if self.matches(&TokenKind::Less) {
                type_str.push('<');
                loop {
                    type_str.push_str(&self.parse_type()?);
                    if self.matches(&TokenKind::Comma) {
                        type_str.push_str(", ");
                        continue;
                    }
                    break;
                }
                self.expect(
                    TokenKind::Greater,
                    "Expected '>' after generic type arguments",
                )?;
                type_str.push('>');
            }

            return self.parse_type_postfix(type_str);
        }

        // Check for function type or grouping: (Int, Int) => String or (Int | String)
        if self.check(&TokenKind::LParen) {
            let start = self.peek_span();
            self.advance(); // consume '('

            let mut params = Vec::new();
            if !self.check(&TokenKind::RParen) {
                loop {
                    params.push(self.parse_type()?);
                    if self.matches(&TokenKind::Comma) {
                        continue;
                    }
                    break;
                }
            }
            self.expect(TokenKind::RParen, "Expected ')' after type")?;

            if self.matches(&TokenKind::FatArrow) {
                let return_ty = self.parse_type()?;
                return Ok(format!("({}) => {}", params.join(", "), return_ty));
            } else {
                // Grouping: (Int | String)
                if params.len() != 1 {
                    return Err(ParseError {
                        message: "Invalid type grouping".to_string(),
                        span: start,
                    });
                }
                let type_str = params.into_iter().next().unwrap();
                return self.parse_type_postfix(type_str);
            }
        }

        let mut type_str = match self.advance().kind.clone() {
            TokenKind::Ident(name) => name,
            TokenKind::Null => "Null".to_string(),
            other => {
                return Err(ParseError {
                    message: format!("Expected type name, found {other:?}"),
                    span: self.previous_span(),
                });
            }
        };

        // Check for generic: Promise<Int>
        if self.matches(&TokenKind::Less) {
            type_str.push('<');
            loop {
                type_str.push_str(&self.parse_type()?);
                if self.matches(&TokenKind::Comma) {
                    type_str.push_str(", ");
                    continue;
                }
                break;
            }
            self.expect(
                TokenKind::Greater,
                "Expected '>' after generic type arguments",
            )?;
            type_str.push('>');
        }

        self.parse_type_postfix(type_str)
    }

    fn parse_type_postfix(&mut self, mut type_str: String) -> Result<String, ParseError> {
        loop {
            // Check for optional: Int?
            if self.matches(&TokenKind::Question) {
                type_str.push('?');
                continue;
            }

            // Check for array: Int[]
            if self.matches(&TokenKind::LBracket) {
                self.expect(TokenKind::RBracket, "Expected ']' after '[' in type")?;
                type_str.push_str("[]");
                continue;
            }
            break;
        }
        Ok(type_str)
    }

    fn parse_lambda_expr(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
        start_span: Span,
    ) -> Result<Expr, ParseError> {
        let params = self.parse_lambda_params()?;
        self.expect(TokenKind::Pipe, "Expected '|' after lambda parameters")?;
        self.expect(TokenKind::FatArrow, "Expected '=>' after lambda parameters")?;
        let body = if matches!(self.peek_kind(), TokenKind::LBrace) {
            let block = self.parse_block_stmt()?;
            if let StmtKind::Block(stmts) = block.kind {
                self.wrap_expr(ExprKind::Block(stmts), block.span)
            } else {
                unreachable!()
            }
        } else {
            self.parse_expr()?
        };
        Ok(self.wrap_expr(
            ExprKind::Lambda {
                params,
                body: Box::new(body),
                return_ty: None,
                annotations,
            },
            start_span,
        ))
    }

    fn parse_fn_expr(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
        start_span: Span,
    ) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LParen, "Expected '(' after 'fn'")?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen, "Expected ')' after parameters")?;
        let return_ty = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = if self.matches(&TokenKind::FatArrow) {
            Box::new(self.parse_expr()?)
        } else {
            let block = self.parse_block_stmt()?;
            if let StmtKind::Block(stmts) = block.kind {
                Box::new(self.wrap_expr(ExprKind::Block(stmts), block.span))
            } else {
                unreachable!()
            }
        };
        Ok(self.wrap_expr(
            ExprKind::Lambda {
                params,
                body,
                return_ty,
                annotations,
            },
            start_span,
        ))
    }

    fn parse_if_expr(&mut self, start_span: pleno_ast::Span) -> Result<Expr, ParseError> {
        self.expect(TokenKind::LParen, "Expected '(' after 'if'")?;
        let condition = self.parse_expr()?;
        self.expect(TokenKind::RParen, "Expected ')' after condition")?;

        let then_branch = self.parse_expr()?;

        self.expect(TokenKind::KeywordElse, "Expected 'else' in if-expression")?;

        let else_branch = self.parse_expr()?;

        Ok(self.wrap_expr(
            ExprKind::If {
                condition: Box::new(condition),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            },
            start_span,
        ))
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
        (TokenKind::KeywordGet, Some(TokenKind::Ident(_)), Some(TokenKind::LParen))
            if name == "get" =>
        {
            true
        }
        (TokenKind::KeywordSet, Some(TokenKind::Ident(_)), Some(TokenKind::LParen))
            if name == "set" =>
        {
            true
        }
        _ => false,
    }
}
