use pleno_ast::{BinaryOp, Comment, Expr, ExprKind, Literal, Program, Span, Stmt, StmtKind};
use pleno_lexer::{Token, TokenKind};
use std::collections::HashMap;

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
    pub hint: Option<String>,
}

impl pleno_diagnostics::ToDiagnostic for ParseError {
    fn to_diagnostic(&self) -> pleno_diagnostics::Diagnostic {
        let mut diagnostic =
            pleno_diagnostics::Diagnostic::error(&self.message).with_label(self.span, "");
        if let Some(hint) = &self.hint {
            diagnostic = diagnostic.with_note(hint);
        }
        diagnostic
    }
}

/// The most operators and calls one statement may chain (`1 + 2 + ...`). Far
/// more than people write; it keeps the trees the compiler walks within every
/// stack the tools run on. Compiling one link takes about 2 KB of stack in a
/// release build (about 40 KB in a debug build): the tools run on 64 MB
/// threads, the fuzzers on 8 MB and the playground on 8 MB (.cargo/config.toml).
const MAX_CHAIN_LENGTH: usize = 1_000;

/// The error for code nested deeper than the parser's depth limit (about 40
/// levels of brackets in one expression).
const NESTED_TOO_DEEPLY: &str =
    "code nested too deeply: split it into smaller expressions or functions";

pub struct Parser {
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    token_docs: HashMap<usize, String>,
    index: usize,
    line_starts: Vec<usize>,
    last_stmt_end_line: usize,
    depth: usize,
    max_depth: usize,
    /// Operators and calls chained in the current statement (see `extend_chain`).
    chain_length: usize,
}

struct FunctionDeclarationConfig {
    kind: pleno_ast::FunctionKind,
    allow_modifiers: bool,
    visibility: Option<pleno_ast::Visibility>,
    is_static: Option<bool>,
    annotations: Vec<pleno_ast::Annotation>,
    is_async: bool,
    is_abstract: bool,
    doc: Option<String>,
    member_start: Option<usize>,
}

struct AccessorDeclarationConfig {
    kind: pleno_ast::FunctionKind,
    visibility: Option<pleno_ast::Visibility>,
    is_static: bool,
    is_abstract: bool,
    annotations: Vec<pleno_ast::Annotation>,
    doc: Option<String>,
    start_pos: usize,
}

impl Parser {
    fn error(&self, message: String, span: Span) -> ParseError {
        ParseError {
            message,
            span,
            hint: None,
        }
    }

    fn error_at_current(&self, message: String) -> ParseError {
        ParseError {
            message,
            span: self.peek_span(),
            hint: None,
        }
    }

    fn error_with_hint(&self, message: String, span: Span, hint: String) -> ParseError {
        ParseError {
            message,
            span,
            hint: Some(hint),
        }
    }

    fn enter_recursive(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > self.max_depth {
            return Err(self.error_at_current(NESTED_TOO_DEEPLY.to_string()));
        }
        Ok(())
    }

    fn leave_recursive(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
    }

    pub fn new(raw_tokens: Vec<Token>, source: &str) -> Self {
        let mut tokens = Vec::new();
        let mut comments = Vec::new();
        let mut token_docs = HashMap::new();
        let mut current_doc = Vec::new();

        for token in raw_tokens {
            if let TokenKind::Comment(content, kind) = token.kind {
                if kind == pleno_ast::CommentKind::Doc {
                    current_doc.push(content.clone());
                }
                // Doc comments are also kept as comments so the formatter
                // and the LSP see them.
                comments.push(Comment {
                    span: token.span,
                    content,
                    kind,
                });
            } else {
                if !current_doc.is_empty() {
                    token_docs.insert(tokens.len(), current_doc.join("\n"));
                    current_doc.clear();
                }
                tokens.push(token);
            }
        }

        // Compute line starts from source
        let mut line_starts = vec![0];
        for (i, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(i + 1);
            }
        }

        Self {
            tokens,
            comments,
            token_docs,
            index: 0,
            line_starts,
            last_stmt_end_line: 0,
            depth: 0,
            chain_length: 0,
            // Each nesting level of source code uses about three units. The limit keeps
            // the recursive parser within a 2 MB thread stack in debug builds.
            max_depth: 128,
        }
    }

    fn get_line(&self, offset: usize) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(line_idx) => line_idx + 1,
            Err(next_line_idx) => next_line_idx,
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

    fn wrap_pattern(&self, kind: pleno_ast::PatternKind, start: Span) -> pleno_ast::Pattern {
        pleno_ast::Pattern::new(kind, Span::new(start.start, self.previous_span().end))
    }

    fn wrap_stmt(&mut self, kind: StmtKind, start: Span) -> Stmt {
        let span = Span::new(start.start, self.previous_span().end);
        let start_line = self.get_line(span.start);

        let leading_blank_lines =
            if self.last_stmt_end_line > 0 && start_line > self.last_stmt_end_line {
                (start_line - self.last_stmt_end_line - 1).min(255) as u8
            } else {
                0
            };

        let end_line = self.get_line(span.end);

        // Find trailing comment on the same line
        let trailing_comment_idx = self.comments.iter().position(|c| {
            let comment_line = self.get_line(c.span.start);
            comment_line == end_line && c.span.start > span.end
        });

        let trailing_comment = if let Some(idx) = trailing_comment_idx {
            let comment = self.comments.remove(idx);
            Some(comment.content)
        } else {
            None
        };

        self.last_stmt_end_line = end_line;

        Stmt {
            kind,
            span,
            leading_blank_lines,
            trailing_comment,
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.enter_recursive()?;
        // Each statement has its own budget of chained operators and calls;
        // a lambda body inside an expression starts a fresh one.
        let outer_chain = std::mem::replace(&mut self.chain_length, 0);
        let res = self.parse_stmt_inner();
        self.chain_length = outer_chain;
        self.leave_recursive();
        res
    }

    /// Counts one more link of an operator or call chain (`a + b + c`,
    /// `x.f().g()`) in the current statement. Chains are parsed in loops, not
    /// by recursion, but they still make a tree as deep as they are long, which
    /// the compiler and the other tools walk recursively.
    fn extend_chain(&mut self) -> Result<(), ParseError> {
        self.chain_length += 1;
        if self.chain_length > MAX_CHAIN_LENGTH {
            return Err(self.error_at_current(format!(
                "expression too long: more than {MAX_CHAIN_LENGTH} operators and calls in one statement; split it into smaller ones"
            )));
        }
        Ok(())
    }

    fn parse_stmt_inner(&mut self) -> Result<Stmt, ParseError> {
        let doc = self.token_docs.get(&self.index).cloned();
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
            TokenKind::KeywordModule => self.parse_module_stmt(doc),
            TokenKind::KeywordLet => self.parse_let_stmt(doc),
            TokenKind::KeywordConst => self.parse_const_stmt(doc),
            TokenKind::KeywordMacro => self.parse_macro_decl_stmt(doc),
            TokenKind::KeywordIf => self.parse_if_stmt(),
            TokenKind::KeywordWhile => self.parse_while_stmt(),
            TokenKind::KeywordFor => self.parse_for_stmt(),
            TokenKind::KeywordTry => self.parse_try_stmt(),
            TokenKind::KeywordWith => self.parse_with_stmt(),
            TokenKind::KeywordThrow => self.parse_throw_stmt(),
            TokenKind::KeywordImport => self.parse_import_stmt(),
            TokenKind::KeywordExport => self.parse_export_stmt(annotations, doc),
            TokenKind::KeywordEnum => self.parse_enum_declaration(annotations, doc),
            TokenKind::KeywordExtend => self.parse_extend_stmt(doc),
            TokenKind::KeywordClass => self.parse_class_stmt(annotations, false, doc),
            TokenKind::KeywordAbstract => {
                self.advance();
                match self.peek_kind() {
                    TokenKind::KeywordClass => self.parse_class_stmt(annotations, true, doc),
                    other => Err(self.error(
                        format!("Expected 'class' after 'abstract', found {other:?}"),
                        self.peek_span(),
                    )),
                }
            }
            TokenKind::KeywordInterface => self.parse_interface_stmt(annotations, doc),
            TokenKind::KeywordFn => self.parse_fn_stmt(annotations, doc),
            TokenKind::KeywordType => self.parse_type_alias_stmt(doc),
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
                        doc,
                        member_start: None,
                    })?;
                    Ok(self.wrap_stmt(
                        StmtKind::Fn {
                            name: decl.name,
                            name_span: decl.name_span,
                            generics: decl.generics,
                            params: decl.params,
                            return_ty: decl.return_ty,
                            body: decl.body,
                            annotations: decl.annotations,
                            is_async: true,
                            doc: decl.doc,
                        },
                        start_span,
                    ))
                } else {
                    Err(self.error(
                        "Expected 'fn' after 'async'".to_string(),
                        self.previous_span(),
                    ))
                }
            }
            TokenKind::KeywordReturn => self.parse_return_stmt(),
            TokenKind::KeywordBreak => self.parse_break_stmt(),
            TokenKind::KeywordContinue => self.parse_continue_stmt(),
            TokenKind::LBrace => self.parse_block_stmt(),
            _ => {
                if !annotations.is_empty() {
                    return Err(self.error(
                        "Annotations are only allowed on functions, classes, and fields"
                            .to_string(),
                        self.previous_span(),
                    ));
                }
                let expr = self.parse_expr()?;
                if self.is_assignment_op() {
                    let stmt = self.parse_assignment(expr, start_span)?;
                    self.expect(TokenKind::Semicolon, "Expected ';' after assignment")?;
                    return Ok(stmt);
                }
                if self.matches(&TokenKind::Semicolon)
                    || matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof)
                    || matches!(expr.kind, ExprKind::Match { .. } | ExprKind::Block(_))
                {
                    Ok(self.wrap_stmt(StmtKind::Expr(expr), start_span))
                } else {
                    Err(self.error(
                        "Expected ';' after expression".to_string(),
                        self.previous_span(),
                    ))
                }
            }
        }
    }

    fn parse_let_stmt(&mut self, doc: Option<String>) -> Result<Stmt, ParseError> {
        let stmt = self.parse_let_declaration(false, doc)?;
        self.expect_with_hint(
            TokenKind::Semicolon,
            "Expected ';' after let statement",
            "Statements must end with a semicolon. Add ';' at the end.",
        )?;
        Ok(stmt)
    }

    fn parse_let_declaration(
        &mut self,
        forced_mutable: bool,
        doc: Option<String>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // let
        let is_lazy = self.matches(&TokenKind::KeywordLazy);
        let mutable = self.matches(&TokenKind::KeywordMut) || forced_mutable;
        let is_deep = self.matches(&TokenKind::KeywordDeep);
        if matches!(self.peek_kind(), TokenKind::LParen | TokenKind::LBracket) {
            if is_lazy || is_deep {
                let modifier = if is_lazy { "lazy" } else { "deep" };
                return Err(self.error(
                    format!("'let {modifier}' cannot destructure; bind a single name"),
                    self.peek_span(),
                ));
            }
            let pattern = self.parse_binding_pattern()?;
            let ty = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            if !self.matches(&TokenKind::Equal) {
                return Err(self.error(
                    "A destructuring 'let' needs a value: let (a, b) = pair;".to_string(),
                    self.peek_span(),
                ));
            }
            let value = self.parse_expr()?;
            return Ok(self.wrap_stmt(
                StmtKind::LetPattern {
                    pattern,
                    mutable,
                    ty,
                    value,
                    doc,
                },
                start_span,
            ));
        }
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            kind if contextual_keyword_name(&kind).is_some() => contextual_keyword_name(&kind)
                .unwrap_or_default()
                .to_string(),
            other => {
                return Err(self.error(
                    format!("Expected identifier after 'let', found {other:?}"),
                    self.previous_span(),
                ));
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
                name_span,
                mutable,
                ty,
                value,
                is_const: false,
                is_deep,
                is_lazy,
                doc,
            },
            start_span,
        ))
    }

    /// Parses the left-hand side of a destructuring `let`: a name, `_`,
    /// a tuple `(a, b)` or an array `[a, b, ...rest]`, nested freely.
    fn parse_binding_pattern(&mut self) -> Result<pleno_ast::BindingPattern, ParseError> {
        self.enter_recursive()?;
        let res = self.parse_binding_pattern_inner();
        self.leave_recursive();
        res
    }

    fn parse_binding_pattern_inner(&mut self) -> Result<pleno_ast::BindingPattern, ParseError> {
        use pleno_ast::{BindingPattern, BindingPatternKind};
        let start_span = self.peek_span();
        match self.peek_kind().clone() {
            TokenKind::Underscore => {
                self.advance();
                Ok(BindingPattern::new(BindingPatternKind::Ignore, start_span))
            }
            TokenKind::Ident(name) => {
                self.advance();
                Ok(BindingPattern::new(
                    BindingPatternKind::Name(name),
                    start_span,
                ))
            }
            kind if contextual_keyword_name(&kind).is_some() => {
                self.advance();
                let name = contextual_keyword_name(&kind)
                    .unwrap_or_default()
                    .to_string();
                Ok(BindingPattern::new(
                    BindingPatternKind::Name(name),
                    start_span,
                ))
            }
            TokenKind::LParen => {
                self.advance();
                let mut items = Vec::new();
                let mut trailing_comma = false;
                while !self.check(&TokenKind::RParen) {
                    if matches!(self.peek_kind(), TokenKind::Ellipsis | TokenKind::DotDot) {
                        return Err(self.error(
                            "A rest element '...' is only allowed in array patterns".to_string(),
                            self.peek_span(),
                        ));
                    }
                    items.push(self.parse_binding_pattern()?);
                    trailing_comma = self.matches(&TokenKind::Comma);
                    if !trailing_comma {
                        break;
                    }
                }
                self.expect(TokenKind::RParen, "Expected ')' after tuple pattern")?;
                if items.len() == 1 && !trailing_comma {
                    return Err(self.error(
                        "A tuple pattern with one element needs a trailing comma: (a,)".to_string(),
                        Span::new(start_span.start, self.previous_span().end),
                    ));
                }
                Ok(BindingPattern::new(
                    BindingPatternKind::Tuple(items),
                    Span::new(start_span.start, self.previous_span().end),
                ))
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                let mut rest = None;
                while !self.check(&TokenKind::RBracket) {
                    if matches!(self.peek_kind(), TokenKind::Ellipsis | TokenKind::DotDot) {
                        let rest_start = self.peek_span();
                        self.advance();
                        let rest_pattern = match self.peek_kind().clone() {
                            TokenKind::Ident(name) => {
                                let span = self.peek_span();
                                self.advance();
                                BindingPattern::new(BindingPatternKind::Name(name), span)
                            }
                            TokenKind::Underscore => {
                                self.advance();
                                BindingPattern::new(BindingPatternKind::Ignore, rest_start)
                            }
                            _ => BindingPattern::new(BindingPatternKind::Ignore, rest_start),
                        };
                        rest = Some(Box::new(rest_pattern));
                        self.matches(&TokenKind::Comma);
                        if !self.check(&TokenKind::RBracket) {
                            return Err(self.error(
                                "The rest element '...' must be the last element of the pattern"
                                    .to_string(),
                                self.peek_span(),
                            ));
                        }
                        break;
                    }
                    elements.push(self.parse_binding_pattern()?);
                    if !self.matches(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RBracket, "Expected ']' after array pattern")?;
                Ok(BindingPattern::new(
                    BindingPatternKind::Array { elements, rest },
                    Span::new(start_span.start, self.previous_span().end),
                ))
            }
            other => Err(self.error(
                format!(
                    "Expected a name, '_', '(' or '[' in a destructuring pattern, found {other:?}"
                ),
                start_span,
            )),
        }
    }

    fn parse_const_stmt(&mut self, doc: Option<String>) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        let stmt = self.parse_const_decl_internal(doc)?;
        self.expect_with_hint(
            TokenKind::Semicolon,
            "Expected ';' after 'const' statement",
            "Constant declarations must end with a semicolon",
        )?;
        Ok(self.wrap_stmt(stmt, start_span))
    }

    fn parse_const_decl_internal(&mut self, doc: Option<String>) -> Result<StmtKind, ParseError> {
        self.advance(); // consume const
        if self.matches(&TokenKind::KeywordMut) {
            return Err(self.error(
                "'const' cannot be mutable".to_string(),
                self.previous_span(),
            ));
        }

        let is_deep = self.matches(&TokenKind::KeywordDeep);
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(self.error(
                    format!("Expected identifier after 'const', found {other:?}"),
                    self.previous_span(),
                ));
            }
        };
        let ty = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        // A constant can never be assigned later, so it must be initialized here.
        if !self.matches(&TokenKind::Equal) {
            return Err(self.error(format!("const '{name}' requires an initializer"), name_span));
        }
        let value = Some(self.parse_expr()?);
        Ok(StmtKind::Let {
            name,
            name_span,
            mutable: false,
            ty,
            value,
            is_const: true,
            is_deep,
            is_lazy: false,
            doc,
        })
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // 'if'

        let has_paren = self.matches(&TokenKind::LParen);
        let condition = self.parse_expr()?;
        if has_paren {
            self.expect(TokenKind::RParen, "Expected ')' after 'if' condition")?;
        }
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
        if self.is_assignment_op() {
            return self.parse_assignment(expr, start_span);
        }
        Ok(self.wrap_stmt(StmtKind::Expr(expr), start_span))
    }

    fn is_assignment_op(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Equal
                | TokenKind::PlusEqual
                | TokenKind::MinusEqual
                | TokenKind::StarEqual
                | TokenKind::SlashEqual
                | TokenKind::PercentEqual
        )
    }

    /// Parses `target = value` or `target op= value`, with the assignment operator
    /// as the next token. Compound assignments desugar to `target = target op value`.
    fn parse_assignment(&mut self, target: Expr, start_span: Span) -> Result<Stmt, ParseError> {
        let compound_op = match self.advance().kind {
            TokenKind::PlusEqual => Some(BinaryOp::Add),
            TokenKind::MinusEqual => Some(BinaryOp::Subtract),
            TokenKind::StarEqual => Some(BinaryOp::Multiply),
            TokenKind::SlashEqual => Some(BinaryOp::Divide),
            TokenKind::PercentEqual => Some(BinaryOp::Modulo),
            _ => None,
        };
        let rhs = self.parse_expr()?;
        let value = match compound_op {
            Some(op) => {
                let span = Span::new(target.span.start, rhs.span.end);
                Expr::new(
                    ExprKind::Binary {
                        left: Box::new(target.clone()),
                        op,
                        right: Box::new(rhs),
                    },
                    span,
                )
            }
            None => rhs,
        };
        match target.kind {
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
            _ => Err(self.error("Invalid assignment target".to_string(), target.span)),
        }
    }

    fn parse_module_stmt(&mut self, doc: Option<String>) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(self.error(
                    format!("Expected module name, found {other:?}"),
                    self.previous_span(),
                ));
            }
        };

        if self.matches(&TokenKind::Semicolon) {
            Ok(self.wrap_stmt(
                StmtKind::Module {
                    name,
                    name_span,
                    body: None,
                    doc,
                },
                start_span,
            ))
        } else {
            self.expect(TokenKind::LBrace, "Expected '{' or ';' after module name")?;
            let mut body = Vec::new();
            while !self.check(&TokenKind::RBrace) {
                if self.is_at_end() {
                    return Err(
                        self.error("Unterminated module body".to_string(), self.previous_span())
                    );
                }
                body.push(self.parse_stmt()?);
            }
            self.expect(TokenKind::RBrace, "Expected '}' after module body")?;
            Ok(self.wrap_stmt(
                StmtKind::Module {
                    name,
                    name_span,
                    body: Some(body),
                    doc,
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

    fn parse_macro_decl_stmt(&mut self, doc: Option<String>) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // macro
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(self.error(
                    format!("Expected macro name after 'macro', found {other:?}"),
                    self.previous_span(),
                ));
            }
        };
        self.expect(TokenKind::LParen, "Expected '(' after macro name")?;
        let mut params = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RParen) {
            loop {
                let param = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(self.error(
                            format!(
                                "Expected parameter name in macro declaration, found {other:?}"
                            ),
                            self.previous_span(),
                        ));
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
        Ok(self.wrap_stmt(
            StmtKind::MacroDecl {
                name,
                name_span,
                params,
                body,
                doc,
            },
            start_span,
        ))
    }

    fn parse_fn_stmt(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
        doc: Option<String>,
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
            doc,
            member_start: None,
        })?;
        Ok(self.wrap_stmt(
            StmtKind::Fn {
                name: decl.name,
                name_span: decl.name_span,
                generics: decl.generics,
                params: decl.params,
                return_ty: decl.return_ty,
                body: decl.body,
                annotations: decl.annotations,
                is_async: false,
                doc: decl.doc,
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
        self.expect_with_hint(
            TokenKind::Semicolon,
            "Expected ';' after return",
            "Return statements must end with a semicolon",
        )?;
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
        self.expect_with_hint(
            TokenKind::Semicolon,
            "Expected ';' after break",
            "Add ';' after 'break' or 'break label' to complete the statement.",
        )?;
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
        self.expect_with_hint(
            TokenKind::Semicolon,
            "Expected ';' after continue",
            "Add ';' after 'continue' or 'continue label' to complete the statement.",
        )?;
        Ok(self.wrap_stmt(StmtKind::Continue(label), start_span))
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // 'for'

        let has_paren = self.matches(&TokenKind::LParen);

        // Peek ahead to distinguish range loop from C-style loop
        let mut is_range = false;
        let mut lookahead = 0;
        let mut paren_depth = 0;

        while !self.is_at_end() {
            if let Some(kind) = self.peek_n_kind(lookahead) {
                if has_paren {
                    if kind == &TokenKind::LParen {
                        paren_depth += 1;
                    } else if kind == &TokenKind::RParen {
                        if paren_depth == 0 {
                            break;
                        }
                        paren_depth -= 1;
                    }
                } else if kind == &TokenKind::LBrace {
                    break;
                }

                if paren_depth == 0 {
                    if kind == &TokenKind::KeywordIn {
                        is_range = true;
                        break;
                    }
                    if kind == &TokenKind::Semicolon {
                        is_range = false;
                        break;
                    }
                }
            } else {
                break;
            }
            lookahead += 1;
        }

        if is_range {
            let mut names = Vec::new();

            // Optional 'let', 'mut', or 'const' in range loops
            if self.check(&TokenKind::KeywordLet)
                || self.check(&TokenKind::KeywordMut)
                || self.check(&TokenKind::KeywordConst)
            {
                self.advance();
            }

            // `for ((k, v) in pairs)` / `for ([a, b] in rows)`: a destructuring
            // pattern, as in `let (a, b) = pair;`.
            let mut pattern = None;
            if matches!(self.peek_kind(), TokenKind::LParen | TokenKind::LBracket) {
                let parsed = self.parse_binding_pattern()?;
                names.extend(
                    parsed
                        .bindings()
                        .into_iter()
                        .map(|(name, _)| name.to_string()),
                );
                pattern = Some(parsed);
            } else {
                loop {
                    let name = match self.advance().kind.clone() {
                        TokenKind::Ident(name) => name,
                        _ => {
                            return Err(self.error(
                                "Expected identifier in 'for' loop".to_string(),
                                self.previous_span(),
                            ));
                        }
                    };
                    names.push(name);
                    if self.matches(&TokenKind::Comma) {
                        continue;
                    }
                    break;
                }
            }

            self.expect(TokenKind::KeywordIn, "Expected 'in' in 'for' loop")?;
            let iterable = self.parse_expr()?;
            if has_paren {
                self.expect(TokenKind::RParen, "Expected ')' after 'for' loop")?;
            }
            let body = Box::new(self.parse_stmt()?);
            Ok(self.wrap_stmt(
                StmtKind::For {
                    label: None,
                    names,
                    pattern,
                    iterable,
                    body,
                },
                start_span,
            ))
        } else {
            // C-style for loop: for (init; condition; step)
            let init = if self.check(&TokenKind::Semicolon) {
                None
            } else if self.check(&TokenKind::KeywordLet) {
                Some(Box::new(self.parse_let_declaration(true, None)?))
            } else if self.check(&TokenKind::KeywordConst) {
                let decl = self.parse_const_decl_internal(None)?;
                Some(Box::new(self.wrap_stmt(decl, self.peek_span())))
            } else {
                Some(Box::new(self.parse_expr_or_assign()?))
            };

            self.parse_c_for_remainder(init, has_paren, start_span)
        }
    }

    fn parse_c_for_remainder(
        &mut self,
        init: Option<Box<Stmt>>,
        has_paren: bool,
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

        let step = if self.check(&TokenKind::RParen) || self.check(&TokenKind::LBrace) {
            None
        } else {
            Some(Box::new(self.parse_expr_or_assign()?))
        };

        if has_paren {
            self.expect(TokenKind::RParen, "Expected ')' after 'for' clause")?;
        }

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

    fn parse_extend_stmt(&mut self, doc: Option<String>) -> Result<Stmt, ParseError> {
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
                return Err(self.error(
                    format!("Expected extension target name, found {other:?}"),
                    self.previous_span(),
                ));
            }
        };

        // `extend <T> Array<T>`: the arguments name the declared generics; the
        // extension applies to the base type.
        if self.matches(&TokenKind::Less) {
            let mut depth = 1;
            while depth > 0 && !self.is_at_end() {
                match self.advance().kind {
                    TokenKind::Less => depth += 1,
                    TokenKind::Greater => depth -= 1,
                    TokenKind::LBrace | TokenKind::Eof => {
                        return Err(self.error(
                            "Expected '>' after extension target type arguments".to_string(),
                            self.previous_span(),
                        ));
                    }
                    _ => {}
                }
            }
        }

        self.expect(TokenKind::LBrace, "Expected '{' after extension target")?;

        let mut methods = Vec::new();
        while !self.is_at_end() && !matches!(self.peek_kind(), TokenKind::RBrace) {
            let member_doc = self.token_docs.get(&self.index).cloned();
            let member_annotations = self.parse_annotations()?;
            let member_start = self.peek().span.start;
            let decl = self.parse_function_decl(FunctionDeclarationConfig {
                kind: pleno_ast::FunctionKind::Method,
                allow_modifiers: true,
                visibility: None,
                is_static: None,
                annotations: member_annotations,
                is_async: false,
                is_abstract: false,
                doc: member_doc,
                member_start: Some(member_start),
            })?;
            methods.push(decl);
        }

        self.expect(TokenKind::RBrace, "Expected '}' after extension body")?;

        Ok(self.wrap_stmt(
            StmtKind::Extend {
                target,
                generics,
                methods,
                doc,
            },
            start_span,
        ))
    }

    fn parse_interface_stmt(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
        doc: Option<String>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // interface
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(self.error(
                    format!("Expected interface name, found {other:?}"),
                    self.previous_span(),
                ));
            }
        };

        let generics = self.parse_generic_params()?;

        let mut extends = Vec::new();
        if self.matches(&TokenKind::KeywordExtends) {
            loop {
                let parent = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(self.error(
                            format!("Expected extended interface name, found {other:?}"),
                            self.previous_span(),
                        ));
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
        let mut properties = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let member_start = self.peek().span.start;
            let member_doc = self.token_docs.get(&self.index).cloned();
            let member_annotations = self.parse_annotations()?;

            let mut visibility = None;
            let mut is_static = false;
            let mut _is_abstract_member = false;
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
                    TokenKind::KeywordAbstract => {
                        self.advance();
                        _is_abstract_member = true;
                    }
                    _ => break,
                }
            }

            if is_accessor(
                self.peek_kind(),
                self.peek_n_kind(1),
                self.peek_n_kind(2),
                "get",
            ) {
                methods.push(self.parse_accessor_decl(AccessorDeclarationConfig {
                    kind: pleno_ast::FunctionKind::Getter,
                    visibility,
                    is_static,
                    is_abstract: true, // always true for interfaces
                    annotations: member_annotations,
                    doc: member_doc,
                    start_pos: member_start,
                })?);
                continue;
            }
            if is_accessor(
                self.peek_kind(),
                self.peek_n_kind(1),
                self.peek_n_kind(2),
                "set",
            ) {
                methods.push(self.parse_accessor_decl(AccessorDeclarationConfig {
                    kind: pleno_ast::FunctionKind::Setter,
                    visibility,
                    is_static,
                    is_abstract: true,
                    annotations: member_annotations,
                    doc: member_doc,
                    start_pos: member_start,
                })?);
                continue;
            }
            if matches!(self.peek_kind(), TokenKind::KeywordProperty) {
                properties.push(self.parse_property_decl(
                    visibility,
                    is_static,
                    true, // is_abstract
                    member_annotations,
                    member_doc,
                    member_start,
                )?);
                continue;
            }

            let is_async = self.matches(&TokenKind::KeywordAsync);

            if self.matches(&TokenKind::KeywordFn) {
                methods.push(self.parse_function_decl_internal(
                    FunctionDeclarationConfig {
                        kind: pleno_ast::FunctionKind::Method,
                        allow_modifiers: true,
                        visibility,
                        is_static: Some(is_static),
                        annotations: member_annotations,
                        is_async,
                        is_abstract: true, // Always abstract in interface
                        doc: member_doc,
                        member_start: Some(member_start),
                    },
                    false, // Already consumed fn
                )?);
                continue;
            }

            if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                match self.peek_n_kind(1) {
                    Some(&TokenKind::LParen) | Some(&TokenKind::Less) => {
                        methods.push(self.parse_function_decl_internal(
                            FunctionDeclarationConfig {
                                kind: pleno_ast::FunctionKind::Method,
                                allow_modifiers: true,
                                visibility,
                                is_static: Some(is_static),
                                annotations: member_annotations,
                                is_async,
                                is_abstract: true,
                                doc: member_doc,
                                member_start: None,
                            },
                            false,
                        )?);
                        continue;
                    }
                    _ => {}
                }
            }

            return Err(self.error(
                format!("Unexpected token in interface: {:?}", self.peek_kind()),
                self.peek_span(),
            ));
        }
        self.expect(TokenKind::RBrace, "Expected '}' after interface body")?;

        Ok(self.wrap_stmt(
            StmtKind::Interface {
                name,
                name_span,
                generics,
                methods,
                properties,
                extends,
                annotations,
                doc,
            },
            start_span,
        ))
    }

    fn parse_enum_declaration(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
        doc: Option<String>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // enum
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(self.error(
                    format!("Expected enum name, found {other:?}"),
                    self.previous_span(),
                ));
            }
        };

        // Parse optional generic parameters
        let _generics = self.parse_generic_params()?;

        self.expect(TokenKind::LBrace, "Expected '{' before enum body")?;
        let mut variants = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            // Check if we reached methods (denoted by 'fn' or visibility or 'static')
            if matches!(
                self.peek_kind(),
                TokenKind::KeywordFn
                    | TokenKind::KeywordPublic
                    | TokenKind::KeywordPrivate
                    | TokenKind::KeywordProtected
                    | TokenKind::KeywordInternal
                    | TokenKind::KeywordStatic
            ) {
                break;
            }

            let variant_token = self.advance().clone();
            let variant_start = variant_token.span.start;
            let variant_name = match variant_token.kind {
                TokenKind::Ident(value) => value,
                other => {
                    return Err(self.error(
                        format!("Expected variant name, found {other:?}"),
                        variant_token.span,
                    ));
                }
            };

            // Parse optional parameters for this variant
            let params = if self.matches(&TokenKind::LParen) {
                let mut params = Vec::new();
                while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                    let param_name_token = self.advance().clone();
                    let param_name_start = param_name_token.span.start;
                    let param_name = match param_name_token.kind {
                        TokenKind::Ident(value) => value,
                        other => {
                            return Err(self.error(
                                format!("Expected parameter name, found {other:?}"),
                                param_name_token.span,
                            ));
                        }
                    };
                    let ty = if self.matches(&TokenKind::Colon) {
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    let param_end = self.previous_span().end;
                    let param_span = Span::new(param_name_start, param_end);
                    params.push(pleno_ast::Param {
                        span: param_span,
                        name: param_name,
                        name_span: param_name_token.span,
                        ty,
                        default: None,
                        is_variadic: false,
                        is_keyword_only: false,
                        ownership: pleno_ast::Ownership::None,
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
            let value = if self.matches(&TokenKind::Equal) {
                Some(self.parse_expr()?)
            } else {
                None
            };

            let variant_end = self.previous_span().end;
            let variant_span = Span::new(variant_start, variant_end);
            variants.push(pleno_ast::EnumVariant {
                span: variant_span,
                name: variant_name,
                name_span: variant_token.span,
                params,
                value,
                doc: None,
            });

            // Allow trailing comma; `;` ends the variants before the methods.
            self.matches(&TokenKind::Comma);
            if self.matches(&TokenKind::Semicolon) {
                break;
            }
        }

        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let member_start = self.peek().span.start;
            let member_doc = self.token_docs.get(&self.index).cloned();
            let member_annotations = self.parse_annotations()?;
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

            let mut decl = self.parse_function_decl(FunctionDeclarationConfig {
                kind: pleno_ast::FunctionKind::Method,
                allow_modifiers: true,
                visibility,
                is_static: Some(is_static),
                annotations: member_annotations,
                is_async: false, // For now enums don't have async methods easily
                is_abstract: false,
                doc: member_doc,
                member_start: Some(member_start),
            })?;
            decl.is_static = is_static;
            methods.push(decl);
        }

        self.expect(TokenKind::RBrace, "Expected '}' after enum body")?;

        Ok(self.wrap_stmt(
            StmtKind::Enum(pleno_ast::EnumDecl {
                span: Span::new(start_span.start, self.previous_span().end),
                name,
                name_span,
                variants,
                methods,
                annotations,
                doc,
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
        self.expect_with_hint(
            TokenKind::Semicolon,
            "Expected ';' after do-while",
            "do-while loops must end with a semicolon: do { ... } while (cond);",
        )?;
        Ok(self.wrap_stmt(StmtKind::DoWhile { body, condition }, start_span))
    }

    fn parse_try_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let try_branch = Box::new(self.parse_stmt()?);
        let mut catch_branches = Vec::new();
        while self.matches(&TokenKind::KeywordCatch) {
            let catch_start = self.previous_span(); // after 'catch'
            self.expect(TokenKind::LParen, "Expected '(' after 'catch'")?;
            let name_span = self.peek_span();
            let name = match self.advance().kind.clone() {
                TokenKind::Ident(name) => name,
                _ => return Err(self.error_at_current("Expected name after '('".to_string())),
            };
            let ty = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            self.expect(TokenKind::RParen, "Expected ')' after catch parameter")?;
            let body = Box::new(self.parse_stmt()?);
            catch_branches.push(pleno_ast::CatchBlock {
                span: Span::new(catch_start.start, self.previous_span().end),
                name,
                name_span,
                ty,
                body,
            });
        }
        let finally_branch = if self.matches(&TokenKind::KeywordFinally) {
            Some(Box::new(self.parse_stmt()?))
        } else {
            None
        };
        if catch_branches.is_empty() && finally_branch.is_none() {
            return Err(self.error(
                "try statement must have at least one catch or finally block".to_string(),
                self.previous_span(),
            ));
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

    /// `with (name = value, ...) { body }`
    fn parse_with_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // with
        self.expect(TokenKind::LParen, "Expected '(' after 'with'")?;
        let mut resources = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RParen) {
            let name_span = self.peek_span();
            let name = match self.peek_kind().clone() {
                TokenKind::Ident(name) => name,
                kind => match contextual_keyword_name(&kind) {
                    Some(name) => name.to_string(),
                    None => {
                        return Err(self.error(
                            format!("Expected a name in 'with', found {kind:?}"),
                            name_span,
                        ));
                    }
                },
            };
            self.advance();
            self.expect_with_hint(
                TokenKind::Equal,
                "Expected '=' after the name in 'with'",
                "Each resource is a name and a value: with (f = File.open(path)) { ... }",
            )?;
            let value = self.parse_expr()?;
            resources.push(pleno_ast::WithResource {
                name,
                name_span,
                value,
            });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        if resources.is_empty() {
            return Err(self.error(
                "'with' needs at least one resource: with (f = File.open(path)) { ... }"
                    .to_string(),
                self.peek_span(),
            ));
        }
        self.expect(
            TokenKind::RParen,
            "Expected ')' after the resources of 'with'",
        )?;
        if !matches!(self.peek_kind(), TokenKind::LBrace) {
            return Err(self.error(
                "Expected '{' after 'with (...)': the body of 'with' is a block".to_string(),
                self.peek_span(),
            ));
        }
        let body = Box::new(self.parse_stmt()?);
        Ok(self.wrap_stmt(StmtKind::With { resources, body }, start_span))
    }

    fn parse_throw_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let expr = self.parse_expr()?;
        self.expect_with_hint(
            TokenKind::Semicolon,
            "Expected ';' after throw",
            "Throw statements must end with a semicolon: throw error;",
        )?;
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
                            return Err(self.error(
                                format!("Expected import name, found {other:?}"),
                                self.previous_span(),
                            ));
                        }
                    };
                    let alias = if self.matches(&TokenKind::KeywordAs) {
                        match self.advance().kind.clone() {
                            TokenKind::Ident(value) => Some(value),
                            other => {
                                return Err(self.error(
                                    format!("Expected alias name after 'as', found {other:?}"),
                                    self.previous_span(),
                                ));
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
                            return Err(self.error(
                                format!("Expected alias after 'as', found {other:?}"),
                                self.previous_span(),
                            ));
                        }
                    }
                } else {
                    return Err(self.error(
                        "Expected 'as' alias after '*' import".to_string(),
                        self.previous_span(),
                    ));
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
                return Err(self.error(
                    format!("Expected string literal path, found {other:?}"),
                    self.previous_span(),
                ));
            }
        };
        self.expect_with_hint(
            TokenKind::Semicolon,
            "Expected ';' after import",
            "Import statements must end with a semicolon",
        )?;
        Ok(self.wrap_stmt(StmtKind::Import { path, kind }, start_span))
    }

    fn parse_export_stmt(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
        doc: Option<String>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        if self.matches(&TokenKind::LBrace) {
            if !annotations.is_empty() {
                return Err(self.error(
                    "Annotations are not allowed on export lists".to_string(),
                    self.previous_span(),
                ));
            }
            let mut items = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::RBrace) {
                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(self.error(
                            format!("Expected export name, found {other:?}"),
                            self.previous_span(),
                        ));
                    }
                };
                let alias = if self.matches(&TokenKind::KeywordAs) {
                    match self.advance().kind.clone() {
                        TokenKind::Ident(value) => Some(value),
                        other => {
                            return Err(self.error(
                                format!("Expected alias name after 'as', found {other:?}"),
                                self.previous_span(),
                            ));
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
            TokenKind::KeywordFn => self.parse_fn_stmt(annotations, doc)?,
            TokenKind::KeywordClass => self.parse_class_stmt(annotations, false, doc)?,
            TokenKind::KeywordLet => self.parse_let_stmt(doc)?,
            TokenKind::KeywordConst => self.parse_const_stmt(doc)?,
            other => {
                return Err(self.error(
                    format!("Expected declaration after export, found {other:?}"),
                    self.peek().span,
                ));
            }
        };
        Ok(self.wrap_stmt(StmtKind::Export(Box::new(stmt)), start_span))
    }

    fn parse_class_stmt(
        &mut self,
        annotations: Vec<pleno_ast::Annotation>,
        is_abstract: bool,
        doc: Option<String>,
    ) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance();
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(self.error(
                    format!("Expected class name, found {other:?}"),
                    self.previous_span(),
                ));
            }
        };
        let generics = self.parse_generic_params()?;
        let superclass = if self.matches(&TokenKind::KeywordExtends) {
            match self.advance().kind.clone() {
                TokenKind::Ident(value) => Some(value),
                other => {
                    return Err(self.error(
                        format!("Expected superclass name, found {other:?}"),
                        self.previous_span(),
                    ));
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

        self.expect_with_hint(
            TokenKind::LBrace,
            "Expected '{' before class body",
            "Class definitions require a body enclosed in braces: class Name { ... }",
        )?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut nested_classes = Vec::new();
        while !self.is_at_end() && !matches!(self.peek_kind(), TokenKind::RBrace) {
            let member_start = self.peek().span.start;
            let member_doc = self.token_docs.get(&self.index).cloned();
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
                        if self.matches(&TokenKind::Equal) {
                            "[]=".to_string()
                        } else {
                            "[]".to_string()
                        }
                    }
                    other => {
                        return Err(self.error(
                            format!("Expected operator symbol, found {other:?}"),
                            self.peek_span(),
                        ));
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
                let method_span = Span::new(member_start, self.previous_span().end);
                methods.push(pleno_ast::FunctionDecl {
                    span: method_span,
                    name: format!("operator{}", op_symbol),
                    name_span: self.previous_span(),
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
                    doc: member_doc,
                });
                continue;
            }
            if matches!(self.peek_kind(), TokenKind::KeywordFn)
                || matches!(self.peek_kind(), TokenKind::Ident(_))
                || matches!(self.peek_kind(), TokenKind::KeywordAsync)
            {
                let is_async = self.matches(&TokenKind::KeywordAsync);
                if self.matches(&TokenKind::KeywordFn) {
                    methods.push(self.parse_function_decl_internal(
                        FunctionDeclarationConfig {
                            kind: pleno_ast::FunctionKind::Method,
                            allow_modifiers: true,
                            visibility,
                            is_static: Some(is_static),
                            annotations: member_annotations,
                            is_async,
                            is_abstract: is_abstract_member,
                            doc: member_doc,
                            member_start: Some(member_start),
                        },
                        false,
                    )?);
                    continue;
                }

                if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                    match self.peek_n_kind(1) {
                        Some(&TokenKind::LParen) | Some(&TokenKind::Less) => {
                            methods.push(self.parse_function_decl_internal(
                                FunctionDeclarationConfig {
                                    kind: pleno_ast::FunctionKind::Method,
                                    allow_modifiers: true,
                                    visibility,
                                    is_static: Some(is_static),
                                    annotations: member_annotations,
                                    is_async,
                                    is_abstract: is_abstract_member,
                                    doc: member_doc,
                                    member_start: Some(member_start),
                                },
                                false,
                            )?);
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // Relaxed field declaration: visibility static? mut? identifier
            if is_accessor(
                self.peek_kind(),
                self.peek_n_kind(1),
                self.peek_n_kind(2),
                "get",
            ) {
                methods.push(self.parse_accessor_decl(AccessorDeclarationConfig {
                    kind: pleno_ast::FunctionKind::Getter,
                    visibility,
                    is_static,
                    is_abstract: is_abstract_member,
                    annotations: member_annotations,
                    doc: member_doc,
                    start_pos: member_start,
                })?);
                continue;
            }
            if is_accessor(
                self.peek_kind(),
                self.peek_n_kind(1),
                self.peek_n_kind(2),
                "set",
            ) {
                methods.push(self.parse_accessor_decl(AccessorDeclarationConfig {
                    kind: pleno_ast::FunctionKind::Setter,
                    visibility,
                    is_static,
                    is_abstract: is_abstract_member,
                    annotations: member_annotations,
                    doc: member_doc,
                    start_pos: member_start,
                })?);
                continue;
            }
            if matches!(self.peek_kind(), TokenKind::KeywordProperty) {
                properties.push(self.parse_property_decl(
                    visibility,
                    is_static,
                    is_abstract_member,
                    member_annotations,
                    member_doc,
                    member_start,
                )?);
                continue;
            }
            if matches!(self.peek_kind(), TokenKind::KeywordClass) {
                nested_classes.push(self.parse_class_stmt(
                    member_annotations,
                    false,
                    member_doc,
                )?);
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
                let constructor_span = Span::new(member_start, self.previous_span().end);
                methods.push(pleno_ast::FunctionDecl {
                    span: constructor_span,
                    name,
                    name_span: self.tokens[self.index - 5].span, // 'constructor' keyword. Approximate.
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
                    doc: member_doc,
                });
                continue;
            }
            let name_token = self.peek().clone();
            let name = match self.advance().kind.clone() {
                TokenKind::Ident(value) => value,
                other => {
                    return Err(self.error(
                        format!("Expected field name in class body, found {other:?}"),
                        self.previous_span(),
                    ));
                }
            };
            let name_span = name_token.span;

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
            let field_span = Span::new(member_start, self.previous_span().end);
            fields.push(pleno_ast::FieldDecl {
                span: field_span,
                name,
                name_span,
                ty,
                default,
                is_static,
                is_const: is_const && !is_mut,
                visibility,
                annotations: member_annotations,
                doc: member_doc,
            });
        }
        self.expect_with_hint(
            TokenKind::RBrace,
            "Expected '}' after class body",
            "Make sure all class members are complete and close with '}'",
        )?;
        Ok(self.wrap_stmt(
            StmtKind::Class {
                name,
                name_span,
                generics,
                fields,
                methods,
                properties,
                superclass,
                implements,
                is_abstract,
                annotations,
                nested_classes,
                doc,
            },
            start_span,
        ))
    }

    fn parse_property_decl(
        &mut self,
        visibility: Option<pleno_ast::Visibility>,
        is_static: bool,
        is_abstract: bool,
        annotations: Vec<pleno_ast::Annotation>,
        doc: Option<String>,
        start_pos: usize,
    ) -> Result<pleno_ast::PropertyDecl, ParseError> {
        self.advance(); // consume 'property'
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            TokenKind::KeywordGet => "get".to_string(),
            TokenKind::KeywordSet => "set".to_string(),
            other => {
                return Err(self.error(
                    format!("Expected property name, found {other:?}"),
                    self.previous_span(),
                ));
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
                    return Err(self.error(
                        "Property already has a getter".to_string(),
                        self.previous_span(),
                    ));
                }
                let body = if is_abstract && !self.check(&TokenKind::LBrace) {
                    self.expect(TokenKind::Semicolon, "Expected ';' after abstract getter")?;
                    Box::new(self.wrap_stmt(StmtKind::Block(Vec::new()), self.previous_span()))
                } else {
                    Box::new(self.parse_block_stmt()?)
                };
                let getter_span = Span::new(self.peek().span.start, self.previous_span().end);
                getter = Some(pleno_ast::FunctionDecl {
                    span: getter_span,
                    name: format!("get_{}", name),
                    name_span: self.previous_span(), // 'get' keyword
                    generics: Vec::new(),
                    params: Vec::new(),
                    return_ty: ty.clone(),
                    body,
                    is_static,
                    is_abstract,
                    is_async: false,
                    visibility: visibility.clone(),
                    kind: pleno_ast::FunctionKind::Getter,
                    annotations: Vec::new(),
                    doc: None,
                });
            } else if self.matches(&TokenKind::KeywordSet) {
                if setter.is_some() {
                    return Err(self.error(
                        "Property already has a setter".to_string(),
                        self.previous_span(),
                    ));
                }
                self.expect(TokenKind::LParen, "Expected '(' after 'set'")?;
                let param_name_span = self.peek_span();
                let param_name = match self.advance().kind.clone() {
                    TokenKind::Ident(value) => value,
                    other => {
                        return Err(self.error(
                            format!("Expected parameter name in setter, found {other:?}"),
                            self.previous_span(),
                        ));
                    }
                };
                self.expect(TokenKind::RParen, "Expected ')' after parameter name")?;
                let body = if is_abstract && !self.check(&TokenKind::LBrace) {
                    self.expect(TokenKind::Semicolon, "Expected ';' after abstract setter")?;
                    Box::new(self.wrap_stmt(StmtKind::Block(Vec::new()), self.previous_span()))
                } else {
                    Box::new(self.parse_block_stmt()?)
                };
                let setter_span = Span::new(self.peek().span.start, self.previous_span().end);
                setter = Some(pleno_ast::FunctionDecl {
                    span: setter_span,
                    name: format!("set_{}", name),
                    name_span: self.tokens[self.index - 7].span, // 'set' keyword. Approximate.
                    generics: Vec::new(),
                    params: vec![pleno_ast::Param {
                        span: param_name_span,
                        name: param_name,
                        name_span: param_name_span,
                        ty: ty.clone(),
                        default: None,
                        is_variadic: false,
                        is_keyword_only: false,
                        ownership: pleno_ast::Ownership::None,
                    }],
                    return_ty: Some("Void".to_string()),
                    body,
                    is_static,
                    is_abstract,
                    is_async: false,
                    visibility: visibility.clone(),
                    kind: pleno_ast::FunctionKind::Setter,
                    annotations: Vec::new(),
                    doc: None,
                });
            } else {
                return Err(self.error(
                    format!(
                        "Expected 'get' or 'set' in property body, found {:?}",
                        self.peek_kind()
                    ),
                    self.peek_span(),
                ));
            }
        }

        self.expect(TokenKind::RBrace, "Expected '}' after property body")?;

        let end_pos = self.previous_span().end;
        Ok(pleno_ast::PropertyDecl {
            span: Span::new(start_pos, end_pos),
            name,
            name_span,
            ty,
            getter,
            setter,
            is_static,
            visibility,
            annotations,
            doc,
        })
    }

    fn parse_generic_params(&mut self) -> Result<Vec<pleno_ast::GenericParam>, ParseError> {
        if !self.matches(&TokenKind::Less) {
            return Ok(Vec::new());
        }
        let mut params = Vec::new();
        loop {
            let mut variance = pleno_ast::Variance::Invariant;
            if self.matches(&TokenKind::KeywordOut) {
                variance = pleno_ast::Variance::Covariant;
            } else if self.matches(&TokenKind::KeywordIn) {
                variance = pleno_ast::Variance::Contravariant;
            }

            let name = match self.advance().kind.clone() {
                TokenKind::Ident(name) => name,
                other => {
                    return Err(self.error(
                        format!("Expected generic parameter name, found {other:?}"),
                        self.previous_span(),
                    ));
                }
            };
            let mut constraints = Vec::new();
            if self.matches(&TokenKind::Colon) {
                // Parse constraints
                loop {
                    let constraint = match self.advance().kind.clone() {
                        TokenKind::Ident(name) => name,
                        other => {
                            return Err(self.error(
                                format!("Expected constraint name, found {other:?}"),
                                self.previous_span(),
                            ));
                        }
                    };
                    constraints.push(constraint);
                    // Support both + and & for constraints (specs use &, but + is common)
                    if !self.matches(&TokenKind::Plus) && !self.matches(&TokenKind::Ampersand) {
                        break;
                    }
                }
            }
            params.push(pleno_ast::GenericParam {
                name,
                constraints,
                variance,
            });
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
        self.parse_function_decl_internal(config, true)
    }

    fn parse_function_decl_internal(
        &mut self,
        config: FunctionDeclarationConfig,
        consume_fn: bool,
    ) -> Result<pleno_ast::FunctionDecl, ParseError> {
        if consume_fn {
            self.advance();
        }
        let name_token = self.advance().clone();
        let name_start = name_token.span.start;
        let name = match name_token.kind {
            TokenKind::Ident(name) => name,
            TokenKind::KeywordMatch => "match".to_string(),
            TokenKind::KeywordGet => "get".to_string(),
            TokenKind::KeywordSet => "set".to_string(),
            // Like the built-in `Array.with`, a class can have a `with` method.
            TokenKind::KeywordWith => "with".to_string(),
            other => {
                return Err(self.error(
                    format!("Expected function name, found {other:?}"),
                    name_token.span,
                ));
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
            // The `;` after an abstract signature is optional.
            self.matches(&TokenKind::Semicolon);
            let span = self.previous_span();
            Box::new(self.wrap_stmt(StmtKind::Block(Vec::new()), span))
        } else if self.matches(&TokenKind::FatArrow) {
            let expr = self.parse_expr()?;
            self.expect(
                TokenKind::Semicolon,
                "Expected ';' after shorthand function body",
            )?;
            let span = self.previous_span();
            Box::new(self.wrap_stmt(StmtKind::Return(Some(expr)), span))
        } else {
            Box::new(self.parse_block_stmt()?)
        };

        let (visibility, is_static) = if config.allow_modifiers {
            (config.visibility, config.is_static.unwrap_or(false))
        } else {
            if config.visibility.is_some() || config.is_static.unwrap_or(false) {
                return Err(self.error(
                    "Function modifiers are only allowed in classes".to_string(),
                    self.previous_span(),
                ));
            }
            (None, false)
        };
        let decl_end = self.previous_span().end;
        let decl_span = Span::new(config.member_start.unwrap_or(name_start), decl_end);
        Ok(pleno_ast::FunctionDecl {
            span: decl_span,
            name,
            name_span: name_token.span,
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
            doc: config.doc,
        })
    }

    fn parse_accessor_decl(
        &mut self,
        config: AccessorDeclarationConfig,
    ) -> Result<pleno_ast::FunctionDecl, ParseError> {
        let _accessor_token = self.advance().clone(); // get/set
        let kind = config.kind;
        let visibility = config.visibility;
        let is_static = config.is_static;
        let is_abstract = config.is_abstract;
        let annotations = config.annotations;
        let doc = config.doc;
        let start_pos = config.start_pos;
        let name_token = self.advance().clone();
        let name = match name_token.kind {
            TokenKind::Ident(value) => value,
            other => {
                return Err(self.error(
                    format!("Expected accessor name, found {other:?}"),
                    name_token.span,
                ));
            }
        };
        self.expect(TokenKind::LParen, "Expected '(' after accessor name")?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen, "Expected ')' after accessor parameters")?;
        match kind {
            pleno_ast::FunctionKind::Getter if !params.is_empty() => {
                return Err(self.error(
                    "getter cannot take parameters".to_string(),
                    self.previous_span(),
                ));
            }
            pleno_ast::FunctionKind::Setter if params.len() != 1 => {
                return Err(self.error(
                    "setter must take exactly one parameter".to_string(),
                    self.previous_span(),
                ));
            }
            _ => {}
        }
        let body = if is_abstract && !self.check(&TokenKind::LBrace) {
            self.expect(TokenKind::Semicolon, "Expected ';' after abstract accessor")?;
            let span = self.previous_span();
            Box::new(self.wrap_stmt(StmtKind::Block(Vec::new()), span))
        } else {
            Box::new(self.parse_block_stmt()?)
        };
        let decl_end = self.previous_span().end;
        let decl_span = Span::new(start_pos, decl_end);
        Ok(pleno_ast::FunctionDecl {
            span: decl_span,
            name,
            name_span: name_token.span,
            generics: Vec::new(),
            params,
            return_ty: None,
            body,
            is_static,
            is_abstract,
            is_async: false,
            visibility,
            kind,
            annotations,
            doc,
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
                return Err(self.error(
                    format!("Expected annotation name, found {other:?}"),
                    self.previous_span(),
                ));
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
                    return Err(self.error(
                        "duplicate keyword-only marker".to_string(),
                        self.previous_span(),
                    ));
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
                    return Err(self.error(
                        "variadic parameter already defined".to_string(),
                        self.previous_span(),
                    ));
                }
                seen_variadic = true;
                true
            } else {
                false
            };
            let mut ownership = self.parse_ownership_hint();

            let name_token = self.advance().clone();
            let name_start = name_token.span.start;
            let name = match name_token.kind {
                TokenKind::Ident(name) => name,
                TokenKind::Underscore => "_".to_string(),
                kind if contextual_keyword_name(&kind).is_some() => contextual_keyword_name(&kind)
                    .unwrap_or_default()
                    .to_string(),
                other => {
                    return Err(self.error(
                        format!("Expected parameter name, found {other:?}"),
                        name_token.span,
                    ));
                }
            };
            let ty = if self.matches(&TokenKind::Colon) {
                // The hint may also come before the type: `value: move String`,
                // `value: &String`, `value: &mut Array`.
                let hint = self.parse_ownership_hint();
                if hint != pleno_ast::Ownership::None {
                    ownership = hint;
                }
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
                return Err(self.error(
                    "variadic parameter cannot have default".to_string(),
                    self.previous_span(),
                ));
            }
            let param_end = self.previous_span().end;
            let param_span = Span::new(name_start, param_end);
            params.push(pleno_ast::Param {
                span: param_span,
                name,
                name_span: name_token.span,
                ty,
                default,
                is_variadic,
                is_keyword_only: keyword_only,
                ownership,
            });
            if is_variadic && !matches!(self.peek_kind(), TokenKind::RParen) {
                return Err(self.error(
                    "variadic parameter must be last".to_string(),
                    self.peek().span,
                ));
            }
            if !self.matches(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RParen) {
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
            let name_token = self.advance().clone();
            let name_start = name_token.span.start;
            let name = match name_token.kind {
                TokenKind::Ident(name) => name,
                TokenKind::Underscore => "_".to_string(),
                other => {
                    return Err(self.error(
                        format!("Expected parameter name in lambda, found {other:?}"),
                        name_token.span,
                    ));
                }
            };
            // `(x: Int) => ...`: an annotation, checked when the lambda is called.
            let ty = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let param_end = self.previous_span().end;
            let param_span = Span::new(name_start, param_end);
            params.push(pleno_ast::Param {
                span: param_span,
                name,
                name_span: name_token.span,
                ty,
                default: None,
                is_variadic: false,
                is_keyword_only: false,
                ownership: pleno_ast::Ownership::None,
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
        self.enter_recursive()?;
        let res = self.parse_ternary();
        self.leave_recursive();
        res
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
                    else_branch: Some(Box::new(else_branch)),
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
            self.extend_chain()?;
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
            self.extend_chain()?;
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
            self.extend_chain()?;
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
            self.extend_chain()?;
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
        let start_span = expr.span;
        while self.matches(&TokenKind::EqualEqual) || self.matches(&TokenKind::BangEqual) {
            let op = match self.previous().kind {
                TokenKind::EqualEqual => pleno_ast::BinaryOp::Equal,
                TokenKind::BangEqual => pleno_ast::BinaryOp::NotEqual,
                _ => unreachable!(),
            };
            let right = self.parse_is_expr()?;
            self.extend_chain()?;
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

    fn parse_is_expr(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_comparison()?;
        if self.matches(&TokenKind::KeywordIs) {
            let start_span = expr.span;
            let ty = self.parse_type()?;
            self.extend_chain()?;
            expr = self.wrap_expr(
                ExprKind::Is {
                    left: Box::new(expr),
                    ty,
                },
                start_span,
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
            self.extend_chain()?;
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
            self.extend_chain()?;
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
            self.extend_chain()?;
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
        self.enter_recursive()?;
        let res = self.parse_unary_inner();
        self.leave_recursive();
        res
    }

    fn parse_unary_inner(&mut self) -> Result<Expr, ParseError> {
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
        if self.matches(&TokenKind::PlusPlus) {
            let expr = self.parse_unary()?;
            return Ok(self.wrap_expr(
                ExprKind::Unary {
                    op: pleno_ast::UnaryOp::PreIncrement,
                    expr: Box::new(expr),
                },
                start_span,
            ));
        }
        if self.matches(&TokenKind::MinusMinus) {
            let expr = self.parse_unary()?;
            return Ok(self.wrap_expr(
                ExprKind::Unary {
                    op: pleno_ast::UnaryOp::PreDecrement,
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
            let name_token = self.advance().clone();
            let name_start = name_token.span.start;
            let name = match name_token.kind {
                TokenKind::Ident(name) => name,
                TokenKind::Underscore => "_".to_string(),
                other => {
                    return Err(self.error(
                        format!("Expected parameter name in lambda, found {other:?}"),
                        name_token.span,
                    ));
                }
            };
            let param_end = self.previous_span().end;
            let param_span = Span::new(name_start, param_end);
            params.push(pleno_ast::Param {
                span: param_span,
                name,
                name_span: name_token.span,
                ty: None,
                default: None,
                is_variadic: false,
                is_keyword_only: false,
                ownership: pleno_ast::Ownership::None,
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

    // Kept out of `parse_atom_inner`, like `parse_string_literal`.
    #[inline(never)]
    fn parse_async_fn_expr(&mut self, start_span: Span) -> Result<Expr, ParseError> {
        let mut lambda = self.parse_fn_expr(Vec::new(), start_span)?;
        if let ExprKind::Lambda { is_async, .. } = &mut lambda.kind {
            *is_async = true;
        }
        Ok(lambda)
    }

    // Kept out of `parse_atom_inner` so its stack frame, which every level of
    // nesting pays for, stays small.
    #[inline(never)]
    fn parse_string_literal(&mut self, value: String, span: Span) -> Result<Expr, ParseError> {
        if value.contains("${") {
            self.parse_interpolated_string(value, span)
        } else {
            let value = pleno_lexer::unescape_dollar(&value);
            Ok(self.wrap_expr(ExprKind::Literal(Literal::String(value)), span))
        }
    }

    fn parse_interpolated_string(&mut self, value: String, span: Span) -> Result<Expr, ParseError> {
        let mut segments = Vec::new();
        let mut last_idx = 0;
        let bytes = value.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
                // Add preceding literal segment
                if i > last_idx {
                    let literal =
                        pleno_lexer::unescape_dollar(&String::from_utf8_lossy(&bytes[last_idx..i]));
                    segments.push(Expr::new(ExprKind::Literal(Literal::String(literal)), span));
                }

                // Find matching brace
                let start_expr = i + 2;
                let mut brace_count = 1;
                let mut j = start_expr;
                while j < bytes.len() && brace_count > 0 {
                    if bytes[j] == b'{' {
                        brace_count += 1;
                    } else if bytes[j] == b'}' {
                        brace_count -= 1;
                    }
                    j += 1;
                }

                if brace_count > 0 {
                    return Err(self.error("Unterminated interpolation".to_string(), span));
                }

                let expr_str = String::from_utf8_lossy(&bytes[start_expr..j - 1]).to_string();

                // Parse the expression
                let tokens = pleno_lexer::lex(&expr_str).map_err(|e| {
                    self.error(format!("Lex error in interpolation: {}", e.message), span)
                })?;
                let mut sub_parser = Parser::new(tokens, &expr_str);
                // The interpolation is nested in this expression: it shares
                // the depth limit, so `"${"${...}"}"` cannot overflow the stack.
                sub_parser.depth = self.depth + 1;
                sub_parser.max_depth = self.max_depth;
                if sub_parser.depth > sub_parser.max_depth {
                    return Err(self.error_at_current(NESTED_TOO_DEEPLY.to_string()));
                }
                let expr = sub_parser.parse_expr().map_err(|e| {
                    // An error from a nested interpolation already says so.
                    let message = if e.message.contains("in interpolation: ") {
                        e.message
                    } else {
                        format!("Parse error in interpolation: {}", e.message)
                    };
                    self.error(message, span)
                })?;
                segments.push(expr);

                i = j;
                last_idx = i;
            } else {
                i += 1;
            }
        }

        if last_idx < bytes.len() {
            let literal =
                pleno_lexer::unescape_dollar(&String::from_utf8_lossy(&bytes[last_idx..]));
            segments.push(Expr::new(ExprKind::Literal(Literal::String(literal)), span));
        }

        Ok(self.wrap_expr(ExprKind::InterpolatedString(segments), span))
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        self.enter_recursive()?;
        let res = self.parse_atom_inner();
        self.leave_recursive();
        res
    }

    fn parse_atom_inner(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance().clone();
        let start_span = token.span;
        match token.kind.clone() {
            TokenKind::Int(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::Int(value)), start_span))
            }
            TokenKind::Float(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::Float(value)), start_span))
            }
            TokenKind::String(value) => self.parse_string_literal(value, start_span),
            TokenKind::Char(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::Char(value)), start_span))
            }
            TokenKind::Bool(value) => {
                Ok(self.wrap_expr(ExprKind::Literal(Literal::Bool(value)), start_span))
            }
            TokenKind::Null => Ok(self.wrap_expr(ExprKind::Literal(Literal::Null), start_span)),
            TokenKind::Ident(name) => Ok(self.wrap_expr(ExprKind::Ident(name), start_span)),
            kind if contextual_keyword_name(&kind).is_some() => Ok(self.wrap_expr(
                ExprKind::Ident(
                    contextual_keyword_name(&kind)
                        .unwrap_or_default()
                        .to_string(),
                ),
                start_span,
            )),
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
                            if self.check(&TokenKind::RParen) {
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
            // `async fn(...) { ... }`: an anonymous async function.
            TokenKind::KeywordAsync if self.matches(&TokenKind::KeywordFn) => {
                self.parse_async_fn_expr(start_span)
            }
            TokenKind::At => {
                let first_annotation = self.parse_annotation()?;
                let mut annotations = vec![first_annotation];
                annotations.extend(self.parse_annotations()?);
                let start_token = self.advance();
                match start_token.kind {
                    TokenKind::Pipe => self.parse_lambda_expr(annotations, start_span),
                    TokenKind::KeywordFn => self.parse_fn_expr(annotations, start_span),
                    _ => Err(self.error(
                        "Annotations only allowed before lambda functions in expressions"
                            .to_string(),
                        start_span,
                    )),
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
                            is_async: false,
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
                        let name_token = self.advance().clone();
                        let name = match name_token.kind {
                            TokenKind::Ident(name) => name,
                            other => {
                                return Err(self.error(
                                    format!("Expected identifier in named tuple, found {other:?}"),
                                    name_token.span,
                                ));
                            }
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
                            if matches!(self.peek_kind(), TokenKind::RParen) {
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
                    return Err(self.error(
                        "Expected '{' after '$' for macro splice".to_string(),
                        dollar_span,
                    ));
                }
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RBrace, "Expected '}' after macro splice")?;
                Ok(self.wrap_expr(ExprKind::MacroSplice(Box::new(expr)), start_span))
            }
            other => Err(self.error(
                format!("Unexpected token in expression: {other:?}"),
                token.span,
            )),
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
                            return Err(self.error(
                                format!("Expected identifier after '.#', found {other:?}"),
                                self.previous_span(),
                            ));
                        }
                    };
                    self.extend_chain()?;
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
                    TokenKind::Int(val) => val.to_string(),
                    keyword if keyword.keyword_text().is_some() => {
                        keyword.keyword_text().unwrap_or_default().to_string()
                    }
                    // `assert.true(x)`, `assert.null(x)`
                    TokenKind::Bool(value) => value.to_string(),
                    TokenKind::Null => "null".to_string(),
                    other => {
                        return Err(self.error(
                            format!("Expected identifier or index after '.', found {other:?}"),
                            self.previous_span(),
                        ));
                    }
                };
                self.extend_chain()?;
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
                    self.extend_chain()?;
                    expr = self.wrap_expr(
                        ExprKind::SafeCall {
                            callee: Box::new(expr),
                            args,
                        },
                        start_span,
                    );
                    continue;
                }
                if self.matches(&TokenKind::LBracket) {
                    let index = self.parse_expr()?;
                    self.expect(TokenKind::RBracket, "Expected ']' after safe index")?;
                    self.extend_chain()?;
                    expr = self.wrap_expr(
                        ExprKind::SafeIndex {
                            collection: Box::new(expr),
                            index: Box::new(index),
                        },
                        start_span,
                    );
                    continue;
                }
                let name = match self.advance().kind.clone() {
                    TokenKind::Ident(name) => name,
                    TokenKind::Int(val) => val.to_string(),
                    keyword if keyword.keyword_text().is_some() => {
                        keyword.keyword_text().unwrap_or_default().to_string()
                    }
                    other => {
                        return Err(self.error(
                            format!("Expected identifier or index after '?.', found {other:?}"),
                            self.previous_span(),
                        ));
                    }
                };
                self.extend_chain()?;
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
                    let inclusive = self.previous().kind == TokenKind::DotDotEqual;
                    let end = if self.matches(&TokenKind::RBracket) || self.is_range_terminator() {
                        None
                    } else {
                        let expr = Box::new(self.parse_expr()?);
                        self.expect(TokenKind::RBracket, "Expected ']' after slice")?;
                        Some(expr)
                    };
                    self.extend_chain()?;
                    expr = self.wrap_expr(
                        ExprKind::Slice {
                            collection: Box::new(expr),
                            start: None,
                            inclusive: inclusive && end.is_some(),
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
                    let inclusive = self.previous().kind == TokenKind::DotDotEqual;
                    let end = if self.matches(&TokenKind::RBracket) || self.is_range_terminator() {
                        None
                    } else {
                        let expr = Box::new(self.parse_expr()?);
                        self.expect(TokenKind::RBracket, "Expected ']' after slice")?;
                        Some(expr)
                    };
                    self.extend_chain()?;
                    expr = self.wrap_expr(
                        ExprKind::Slice {
                            collection: Box::new(expr),
                            start: Some(Box::new(start_idx)),
                            inclusive: inclusive && end.is_some(),
                            end,
                        },
                        start_span,
                    );
                    continue;
                }

                // Regular index access
                self.expect(TokenKind::RBracket, "Expected ']' after index")?;
                self.extend_chain()?;
                expr = self.wrap_expr(
                    ExprKind::Index {
                        collection: Box::new(expr),
                        index: Box::new(start_idx),
                    },
                    start_span,
                );
                continue;
            }
            if self.check(&TokenKind::Less) && self.is_likely_specialization() {
                self.advance(); // consume '<'
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
                self.extend_chain()?;
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
                // `f(x)` and `a.m(x)` are one link (the `.m` was counted);
                // a call on a call, `f(1)(2)`, adds one.
                if !matches!(
                    expr.kind,
                    ExprKind::Ident(_) | ExprKind::Property { .. } | ExprKind::SafeProperty { .. }
                ) {
                    self.extend_chain()?;
                }
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
                    self.extend_chain()?;
                    expr = self.wrap_expr(ExprKind::MacroCall { name, args }, start_span);
                    continue;
                }
                self.extend_chain()?;
                expr = self.wrap_expr(ExprKind::AssertNotNull(Box::new(expr)), start_span);
                continue;
            }
            if self.matches(&TokenKind::PlusPlus) {
                self.extend_chain()?;
                expr = self.wrap_expr(
                    ExprKind::Unary {
                        op: pleno_ast::UnaryOp::PostIncrement,
                        expr: Box::new(expr),
                    },
                    start_span,
                );
                continue;
            }
            if self.matches(&TokenKind::MinusMinus) {
                self.extend_chain()?;
                expr = self.wrap_expr(
                    ExprKind::Unary {
                        op: pleno_ast::UnaryOp::PostDecrement,
                        expr: Box::new(expr),
                    },
                    start_span,
                );
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
                    && matches!(
                        self.peek_next_kind(),
                        Some(TokenKind::Colon) | Some(TokenKind::Equal)
                    )
                {
                    let name = match self.advance().kind.clone() {
                        TokenKind::Ident(value) => value,
                        _ => unreachable!("ident checked"),
                    };
                    if *self.peek_kind() == TokenKind::Colon {
                        self.advance();
                    } else {
                        self.expect(TokenKind::Equal, "Expected '=' in named argument")?;
                    }
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
                    // Map keys are strings. A number or Bool literal key is
                    // converted to the string it prints as, the key `m[1]`
                    // reads: `{1: "a"}` is `{"1": "a"}`.
                    let key = match self.advance().kind.clone() {
                        TokenKind::String(value) => pleno_lexer::unescape_dollar(&value),
                        TokenKind::Ident(value) => value,
                        TokenKind::Int(value) => value.to_string(),
                        TokenKind::Float(value) => value.to_string(),
                        TokenKind::Bool(value) => value.to_string(),
                        TokenKind::Minus
                            if matches!(
                                self.peek_kind(),
                                TokenKind::Int(_) | TokenKind::Float(_)
                            ) =>
                        {
                            match self.advance().kind.clone() {
                                TokenKind::Int(value) => (-value).to_string(),
                                TokenKind::Float(value) => (-value).to_string(),
                                _ => unreachable!("checked by the guard"),
                            }
                        }
                        keyword if keyword.keyword_text().is_some() => {
                            keyword.keyword_text().unwrap_or_default().to_string()
                        }
                        other => {
                            return Err(self.error(
                                format!(
                                    "Expected a string, number, Bool or identifier key in map, found {other:?}"
                                ),
                                self.previous_span(),
                            ));
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
            self.matches(&TokenKind::KeywordCase); // optional 'case'

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
                let start = patterns[0].span;
                let end = patterns.last().unwrap().span;
                pleno_ast::Pattern::new(
                    pleno_ast::PatternKind::Or(patterns),
                    Span::new(start.start, end.end),
                )
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
            let body = self.parse_match_arm_body()?;
            arms.push(pleno_ast::MatchArm {
                span: Span::new(pattern.span.start, self.previous_span().end),
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

    /// A match arm's body: an expression, or `return`/`throw` as in
    /// `case 0 => return this.x,`, which becomes a one-statement block.
    fn parse_match_arm_body(&mut self) -> Result<Expr, ParseError> {
        let start_span = self.peek_span();
        let kind = match self.peek_kind() {
            TokenKind::KeywordReturn => {
                self.advance();
                let value = if matches!(self.peek_kind(), TokenKind::Comma | TokenKind::RBrace) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                StmtKind::Return(value)
            }
            TokenKind::KeywordThrow => {
                self.advance();
                StmtKind::Throw(self.parse_expr()?)
            }
            _ => return self.parse_expr(),
        };
        let stmt = self.wrap_stmt(kind, start_span);
        Ok(self.wrap_expr(ExprKind::Block(vec![stmt]), start_span))
    }

    fn parse_pattern(&mut self) -> Result<pleno_ast::Pattern, ParseError> {
        self.enter_recursive()?;
        let res = self.parse_pattern_inner();
        self.leave_recursive();
        res
    }

    fn parse_pattern_inner(&mut self) -> Result<pleno_ast::Pattern, ParseError> {
        let start_span = self.peek_span();
        let mut patterns = vec![self.parse_primary_pattern()?];

        while self.matches(&TokenKind::Pipe) {
            patterns.push(self.parse_primary_pattern()?);
        }

        if patterns.len() == 1 {
            Ok(patterns.pop().unwrap())
        } else {
            Ok(self.wrap_pattern(pleno_ast::PatternKind::Or(patterns), start_span))
        }
    }

    fn parse_primary_pattern(&mut self) -> Result<pleno_ast::Pattern, ParseError> {
        self.enter_recursive()?;
        let res = self.parse_primary_pattern_inner();
        self.leave_recursive();
        res
    }

    fn parse_primary_pattern_inner(&mut self) -> Result<pleno_ast::Pattern, ParseError> {
        let start_span = self.peek_span();
        let token = self.advance();
        match token.kind.clone() {
            TokenKind::Underscore => {
                Ok(self.wrap_pattern(pleno_ast::PatternKind::Underscore, start_span))
            }
            TokenKind::Ident(mut name) => {
                let mut qualified = false;
                while self.matches(&TokenKind::Dot) {
                    qualified = true;
                    if let TokenKind::Ident(subname) = self.advance().kind.clone() {
                        name = subname;
                    } else {
                        return Err(self.error(
                            "Expected variant name after '.'".to_string(),
                            self.previous_span(),
                        ));
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
                            if matches!(self.peek_kind(), TokenKind::RParen) {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen, "Expected ')' after variant pattern")?;
                    return Ok(self.wrap_pattern(
                        pleno_ast::PatternKind::Variant(name, patterns),
                        start_span,
                    ));
                }

                // If it was qualified, it's a variant pattern even without parameters
                if qualified {
                    return Ok(self.wrap_pattern(
                        pleno_ast::PatternKind::Variant(name, Vec::new()),
                        start_span,
                    ));
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
                    return Ok(
                        self.wrap_pattern(pleno_ast::PatternKind::Type(name, binding), start_span)
                    );
                }

                Ok(self.wrap_pattern(pleno_ast::PatternKind::Ident(name), start_span))
            }
            TokenKind::LBrace => {
                let mut patterns = Vec::new();
                let mut rest = None;
                if !matches!(self.peek_kind(), TokenKind::RBrace) {
                    loop {
                        if matches!(self.peek_kind(), TokenKind::DotDot | TokenKind::Ellipsis) {
                            let dot_start = self.peek_span();
                            self.advance();
                            let name = if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                                match &self.advance().kind {
                                    TokenKind::Ident(name) => Some(name.clone()),
                                    _ => unreachable!(),
                                }
                            } else {
                                None
                            };
                            rest = Some(Box::new(
                                self.wrap_pattern(pleno_ast::PatternKind::Rest(name), dot_start),
                            ));
                            if self.matches(&TokenKind::Comma)
                                && !matches!(self.peek_kind(), TokenKind::RBrace)
                            {
                                return Err(self.error(
                                    "Rest pattern must be the last element".to_string(),
                                    self.previous_span(),
                                ));
                            }
                            break;
                        }

                        let kind = self.advance().kind.clone();
                        let field_name = match &kind {
                            TokenKind::Ident(name) => name.clone(),
                            other => {
                                return Err(self.error(
                                    format!(
                                        "Expected field name in object pattern, found {other:?}"
                                    ),
                                    self.previous_span(),
                                ));
                            }
                        };

                        let pattern = if self.matches(&TokenKind::Colon) {
                            self.parse_pattern()?
                        } else {
                            // Shorthand: { x } is { x: x }
                            self.wrap_pattern(
                                pleno_ast::PatternKind::Ident(field_name.clone()),
                                self.previous_span(),
                            )
                        };

                        patterns.push((field_name, pattern));

                        if !self.matches(&TokenKind::Comma) {
                            break;
                        }
                        if matches!(self.peek_kind(), TokenKind::RBrace) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RBrace, "Expected '}' after object pattern")?;
                Ok(self.wrap_pattern(pleno_ast::PatternKind::Object(patterns, rest), start_span))
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
                Ok(self.wrap_pattern(pleno_ast::PatternKind::Tuple(items), start_span))
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
                Ok(self.wrap_pattern(pleno_ast::PatternKind::Tuple(items), start_span))
            }

            TokenKind::Int(val) => Ok(self.wrap_pattern(
                pleno_ast::PatternKind::Literal(pleno_ast::Literal::Int(val)),
                start_span,
            )),
            TokenKind::Float(val) => Ok(self.wrap_pattern(
                pleno_ast::PatternKind::Literal(pleno_ast::Literal::Float(val)),
                start_span,
            )),
            TokenKind::String(val) => Ok(self.wrap_pattern(
                pleno_ast::PatternKind::Literal(pleno_ast::Literal::String(
                    pleno_lexer::unescape_dollar(&val),
                )),
                start_span,
            )),
            TokenKind::Bool(val) => Ok(self.wrap_pattern(
                pleno_ast::PatternKind::Literal(pleno_ast::Literal::Bool(val)),
                start_span,
            )),
            TokenKind::Null => Ok(self.wrap_pattern(
                pleno_ast::PatternKind::Literal(pleno_ast::Literal::Null),
                start_span,
            )),
            TokenKind::DotDot | TokenKind::Ellipsis => {
                let name = if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                    match &self.advance().kind {
                        TokenKind::Ident(name) => Some(name.clone()),
                        _ => unreachable!(),
                    }
                } else {
                    None
                };
                Ok(self.wrap_pattern(pleno_ast::PatternKind::Rest(name), start_span))
            }
            other => Err(self.error(
                format!("Expected pattern, found {other:?}"),
                self.previous_span(),
            )),
        }
    }

    fn is_range_terminator(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Semicolon
                | TokenKind::RParen
                | TokenKind::RBrace
                | TokenKind::RBracket
                | TokenKind::Comma
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
            Err(self.error_at_current(message.to_string()))
        }
    }

    fn expect_with_hint(
        &mut self,
        kind: TokenKind,
        message: &str,
        hint: &str,
    ) -> Result<(), ParseError> {
        if self.peek_kind() == &kind {
            self.advance();
            Ok(())
        } else {
            Err(self.error_with_hint(message.to_string(), self.peek().span, hint.to_string()))
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
        if self.tokens.is_empty() {
            // This should not happen if lexer is correct, but for robustness:
            static EOF: Token = Token {
                kind: TokenKind::Eof,
                span: Span { start: 0, end: 0 },
            };
            return &EOF;
        }
        &self.tokens[self.index.saturating_sub(1)]
    }

    fn previous_span(&self) -> Span {
        self.previous().span
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn parse_type_alias_stmt(&mut self, doc: Option<String>) -> Result<Stmt, ParseError> {
        let start_span = self.peek_span();
        self.advance(); // type
        let name_span = self.peek_span();
        let name = match self.advance().kind.clone() {
            TokenKind::Ident(value) => value,
            other => {
                return Err(self.error(
                    format!("Expected type name after 'type', found {other:?}"),
                    self.previous_span(),
                ));
            }
        };
        let params = self
            .parse_generic_params()?
            .into_iter()
            .map(|param| param.name)
            .collect();
        self.expect(TokenKind::Equal, "Expected '=' after type alias name")?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Semicolon, "Expected ';' after type alias")?;
        Ok(self.wrap_stmt(
            StmtKind::TypeAlias {
                name,
                name_span,
                params,
                ty,
                doc,
            },
            start_span,
        ))
    }

    fn parse_type(&mut self) -> Result<String, ParseError> {
        self.enter_recursive()?;
        let res = self.parse_union_type();
        self.leave_recursive();
        res
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
        self.enter_recursive()?;
        let res = self.parse_base_type_inner();
        self.leave_recursive();
        res
    }

    fn parse_base_type_inner(&mut self) -> Result<String, ParseError> {
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
                    return Err(self.error(
                        format!("Expected type name after '&', found {other:?}"),
                        start_span,
                    ));
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
            self.expect(TokenKind::RParen, "Expected 'RParen' after type list")?;

            if self.matches(&TokenKind::FatArrow) {
                let return_ty = self.parse_type()?;
                return Ok(format!("({}) => {}", params.join(", "), return_ty));
            } else {
                // Tuple type: (Int, String)
                if params.len() >= 2 {
                    return self.parse_type_postfix(format!("({})", params.join(", ")));
                }
                // Grouping: (Int | String)
                if params.len() != 1 {
                    return Err(self.error("Invalid type grouping".to_string(), start));
                }
                let type_str = params.into_iter().next().unwrap();
                return self.parse_type_postfix(type_str);
            }
        }

        // Structural object types: { "key": Type, ... }
        if self.matches(&TokenKind::LBrace) {
            let mut fields = Vec::new();
            if !self.check(&TokenKind::RBrace) {
                loop {
                    let key = match self.advance().kind.clone() {
                        TokenKind::String(name) => format!("\"{}\"", name),
                        TokenKind::Ident(name) => name,
                        other => {
                            return Err(self.error(
                                format!("Expected field name in object type, found {other:?}"),
                                self.previous_span(),
                            ));
                        }
                    };
                    self.expect(TokenKind::Colon, "Expected ':' after field name")?;
                    let ty = self.parse_type()?;
                    fields.push(format!("{}: {}", key, ty));
                    if self.matches(&TokenKind::Comma) {
                        if self.check(&TokenKind::RBrace) {
                            break;
                        }
                        continue;
                    }
                    break;
                }
            }
            self.expect(TokenKind::RBrace, "Expected '}' after object type")?;
            return self.parse_type_postfix(format!("{{ {} }}", fields.join(", ")));
        }

        let mut type_str = match self.advance().kind.clone() {
            TokenKind::Ident(name) => name,
            TokenKind::Null => "Null".to_string(),
            other => {
                return Err(self.error(
                    format!("Expected type name, found {other:?}"),
                    self.previous_span(),
                ));
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
                is_async: false,
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
                is_async: false,
            },
            start_span,
        ))
    }

    fn parse_if_expr(&mut self, start_span: pleno_ast::Span) -> Result<Expr, ParseError> {
        let has_paren = self.matches(&TokenKind::LParen);
        let condition = self.parse_expr()?;
        if has_paren {
            self.expect(TokenKind::RParen, "Expected ')' after 'if' condition")?;
        }

        let then_branch = self.parse_branch_expr()?;

        let else_branch = if self.matches(&TokenKind::KeywordElse) {
            Some(Box::new(self.parse_branch_expr()?))
        } else {
            None
        };

        Ok(self.wrap_expr(
            ExprKind::If {
                condition: Box::new(condition),
                then_branch: Box::new(then_branch),
                else_branch,
            },
            start_span,
        ))
    }

    /// An optional ownership hint: `move`, `&` or `&mut`.
    fn parse_ownership_hint(&mut self) -> pleno_ast::Ownership {
        if self.matches(&TokenKind::KeywordMove) {
            pleno_ast::Ownership::Move
        } else if self.matches(&TokenKind::Ampersand) {
            if self.matches(&TokenKind::KeywordMut) {
                pleno_ast::Ownership::BorrowMut
            } else {
                pleno_ast::Ownership::Borrow
            }
        } else {
            pleno_ast::Ownership::None
        }
    }

    /// A branch of an `if` expression: `{ ... }` is a block whose last expression
    /// is its value (not a set or map literal); anything else is an expression.
    fn parse_branch_expr(&mut self) -> Result<Expr, ParseError> {
        if !matches!(self.peek_kind(), TokenKind::LBrace) {
            return self.parse_expr();
        }
        let block = self.parse_block_stmt()?;
        match block.kind {
            StmtKind::Block(stmts) => Ok(self.wrap_expr(ExprKind::Block(stmts), block.span)),
            _ => Err(self.error("Expected block".to_string(), block.span)),
        }
    }

    fn is_likely_specialization(&self) -> bool {
        if !matches!(self.peek_kind(), TokenKind::Less) {
            return false;
        }

        let mut depth = 0;
        let mut i = 0;
        loop {
            match self.peek_n_kind(i) {
                Some(TokenKind::Less) => depth += 1,
                Some(TokenKind::Greater) => {
                    depth -= 1;
                    if depth == 0 {
                        return true;
                    }
                }
                Some(TokenKind::Semicolon)
                | Some(TokenKind::LBrace)
                | Some(TokenKind::RBrace)
                | Some(TokenKind::Equal)
                | Some(TokenKind::KeywordIf)
                | Some(TokenKind::KeywordElse)
                | Some(TokenKind::FatArrow)
                | None => {
                    return false;
                }
                // Literals, binary operators, and logical operators are not allowed in types
                // When we see &&, ||, etc., this is definitely a comparison, not generics
                Some(TokenKind::Int(_))
                | Some(TokenKind::Float(_))
                | Some(TokenKind::String(_))
                | Some(TokenKind::Plus)
                | Some(TokenKind::Minus)
                | Some(TokenKind::Star)
                | Some(TokenKind::Slash)
                | Some(TokenKind::EqualEqual)
                | Some(TokenKind::BangEqual)
                | Some(TokenKind::LessEqual)
                | Some(TokenKind::GreaterEqual)
                | Some(TokenKind::AndAnd)
                | Some(TokenKind::OrOr)
                | Some(TokenKind::Ampersand)
                | Some(TokenKind::Percent)
                | Some(TokenKind::Question)
                | Some(TokenKind::Colon)
                | Some(TokenKind::RParen) => {
                    return false;
                }
                _ => {}
            }
            i += 1;
            // Limit lookahead to prevent infinite loops or extreme performance hits
            if i > 100 {
                return false;
            }
        }
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

/// Keywords that only have meaning in specific positions (generic variance,
/// accessors, property and operator declarations) and can otherwise be used
/// as variable and parameter names.
fn contextual_keyword_name(kind: &TokenKind) -> Option<&'static str> {
    match kind {
        TokenKind::KeywordOut
        | TokenKind::KeywordGet
        | TokenKind::KeywordSet
        | TokenKind::KeywordProperty
        | TokenKind::KeywordOperator => kind.keyword_text(),
        _ => None,
    }
}
