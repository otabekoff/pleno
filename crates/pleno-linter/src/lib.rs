use pleno_ast::*;
use pleno_lexer::lex;
use pleno_parser::Parser;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct LintRule {
    pub name: String,
    pub description: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LintRelatedInformation {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub file: Option<String>,
}

/// One problem the linter found. Lines are 1-based; columns are 1-based and
/// count UTF-16 code units, as editors do. `end_line` and `end_column` mark the
/// end of the offending text (exclusive).
#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub rule_name: String,
    pub message: String,
    pub severity: Severity,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub related_information: Vec<LintRelatedInformation>,
}

#[derive(Debug, Clone)]
pub struct LintRulesConfig {
    pub warn_unused_vars: bool,
    pub max_line_length: Option<usize>,
    pub warn_trailing_spaces: bool,
    pub warn_semi_spacing: bool,
    pub warn_operator_spacing: bool,
    pub prefer_const: bool,
    pub no_undef: bool,
    pub warn_shadowed_vars: bool,
}

impl Default for LintRulesConfig {
    fn default() -> Self {
        Self {
            warn_unused_vars: true,
            max_line_length: Some(100),
            warn_trailing_spaces: true,
            warn_semi_spacing: true,
            warn_operator_spacing: true,
            prefer_const: false,
            no_undef: true,
            warn_shadowed_vars: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    pub rules: LintRulesConfig,
}

/// A declared name: where it is, and the end of its name (exclusive).
type Declaration = ((usize, usize), (usize, usize));

struct Scope {
    declarations: HashMap<String, Declaration>,
    usages: HashSet<String>,
    /// Parameters declared here, in order.
    params: Vec<String>,
}

impl Scope {
    fn new() -> Self {
        Self {
            declarations: HashMap::new(),
            usages: HashSet::new(),
            params: Vec::new(),
        }
    }
}

struct ScopeTracker {
    scopes: Vec<Scope>,
}

impl ScopeTracker {
    fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn exit_scope(&mut self) -> Scope {
        self.scopes.pop().expect("Exited root scope")
    }

    fn declare(&mut self, name: String, declaration: Declaration) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.declarations.insert(name, declaration);
        }
    }

    fn use_var(&mut self, name: &str) {
        // Find the scope where this variable is declared and mark it as used
        for scope in self.scopes.iter_mut().rev() {
            if scope.declarations.contains_key(name) {
                scope.usages.insert(name.to_string());
                return;
            }
        }
    }
}

/// The globals the standard library defines, which every program can use.
pub fn builtin_globals() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut vm = pleno_vm::VM::new();
        pleno_stdlib::register(&mut vm);
        vm.globals.keys().cloned().collect()
    })
}

pub struct Linter {
    config: LintConfig,
}

impl Default for Linter {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter {
    pub fn new() -> Self {
        Self::with_config(LintConfig::default())
    }

    pub fn with_config(config: LintConfig) -> Self {
        Self { config }
    }

    pub fn lint(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();

        let tokens = match lex(source) {
            Ok(tokens) => tokens,
            Err(e) => {
                diagnostics.push(diagnostic_at(
                    source,
                    "syntax-error",
                    e.message,
                    Severity::Error,
                    e.span,
                ));
                return diagnostics;
            }
        };

        let program = match Parser::new(tokens, source).parse_program() {
            Ok(program) => program,
            Err(e) => {
                diagnostics.push(diagnostic_at(
                    source,
                    "parse-error",
                    e.message,
                    Severity::Error,
                    e.span,
                ));
                return diagnostics;
            }
        };

        let mut visitor = LintVisitor {
            source,
            scope_tracker: ScopeTracker::new(),
            diagnostics: Vec::new(),
            config: &self.config.rules,
            in_interpolation: false,
        };

        visitor.visit_program(&program);
        diagnostics.extend(visitor.diagnostics);

        if self.config.rules.no_undef {
            diagnostics.extend(self.check_undefined(source, &program));
        }
        if let Some(max_length) = self.config.rules.max_line_length {
            diagnostics.extend(self.check_max_line_length(source, max_length));
        }
        if self.config.rules.warn_trailing_spaces {
            diagnostics.extend(self.check_trailing_spaces(source));
        }
        if self.config.rules.warn_semi_spacing {
            diagnostics.extend(self.check_semi_spacing(source));
        }

        diagnostics.sort_by_key(|d| (d.line, d.column));
        diagnostics
    }

    /// Names read but defined nowhere: neither in the program nor by the
    /// standard library. The compiler resolves scopes, so this is the same
    /// answer `pleno check` and running the program give.
    fn check_undefined(&self, source: &str, program: &Program) -> Vec<LintDiagnostic> {
        // Compiling may change the program in place. Unused code is kept, as
        // `pleno check` keeps it: reads there are checked too, and a local
        // nothing reads (often the one a misspelled name meant) stays in scope.
        let mut program = program.clone();
        let mut compiler = pleno_compiler::Compiler::new(source.to_string()).keeping_unused_code();
        if compiler.compile(&mut program).is_err() {
            return Vec::new();
        }
        let builtins = builtin_globals();
        compiler
            .undefined_globals_with_hints(
                |name| builtins.contains(name),
                builtins.iter().map(String::as_str),
            )
            .into_iter()
            .map(|(name, span, hint)| {
                diagnostic_at(
                    source,
                    "no-undef",
                    format!("Usage of undefined variable '{name}'{hint}"),
                    Severity::Error,
                    span,
                )
            })
            .collect()
    }

    fn check_max_line_length(&self, source: &str, max_length: usize) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        for (line_num, line) in source.lines().enumerate() {
            let length = line.chars().count();
            // Comments and URLs cannot always be broken up.
            let trimmed = line.trim_start();
            if length <= max_length
                || trimmed.starts_with("//")
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
                || line.contains("://")
            {
                continue;
            }
            diagnostics.push(LintDiagnostic {
                rule_name: "max-line-length".to_string(),
                message: format!(
                    "Line exceeds maximum line length of {} characters",
                    max_length
                ),
                severity: Severity::Warning,
                line: line_num + 1,
                column: max_length + 1,
                end_line: line_num + 1,
                end_column: length + 1,
                related_information: vec![],
            });
        }
        diagnostics
    }

    fn check_trailing_spaces(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim_end_matches([' ', '\t']);
            if trimmed.len() < line.len() {
                diagnostics.push(LintDiagnostic {
                    rule_name: "no-trailing-spaces".to_string(),
                    message: "Line has trailing spaces".to_string(),
                    severity: Severity::Info,
                    line: line_num + 1,
                    column: trimmed.chars().count() + 1,
                    end_line: line_num + 1,
                    end_column: line.chars().count() + 1,
                    related_information: vec![],
                });
            }
        }
        diagnostics
    }

    fn check_semi_spacing(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        for (line_num, line) in source.lines().enumerate() {
            if line.trim_start().starts_with("//") || line.contains('"') || line.contains('`') {
                continue;
            }
            if let Some(semi_pos) = line.find(';') {
                let after_semi = &line[semi_pos + 1..];
                if !after_semi.is_empty()
                    && !after_semi.starts_with(' ')
                    && !after_semi.trim_start().is_empty()
                {
                    let column = line[..semi_pos].chars().count() + 2;
                    diagnostics.push(LintDiagnostic {
                        rule_name: "semi-spacing".to_string(),
                        message: "Expected space after semicolon".to_string(),
                        severity: Severity::Info,
                        line: line_num + 1,
                        column,
                        end_line: line_num + 1,
                        end_column: column + 1,
                        related_information: vec![],
                    });
                }
            }
        }
        diagnostics
    }
}

/// A diagnostic covering `span`.
fn diagnostic_at(
    source: &str,
    rule_name: &str,
    message: String,
    severity: Severity,
    span: Span,
) -> LintDiagnostic {
    let (line, column) = position_from_offset(source, span.start);
    let (end_line, end_column) = position_from_offset(source, span.end.max(span.start + 1));
    LintDiagnostic {
        rule_name: rule_name.to_string(),
        message,
        severity,
        line,
        column,
        end_line,
        end_column,
        related_information: vec![],
    }
}

struct LintVisitor<'a> {
    source: &'a str,
    scope_tracker: ScopeTracker,
    diagnostics: Vec<LintDiagnostic>,
    config: &'a LintRulesConfig,
    /// Inside `${...}`, which is parsed on its own: spans there are relative
    /// to the interpolation, not to the file.
    in_interpolation: bool,
}

impl<'a> LintVisitor<'a> {
    fn visit_program(&mut self, program: &Program) {
        self.visit_stmts(&program.statements);
        self.check_unused_in_current_scope();
    }

    /// Visits a list of statements. Functions, classes and the other
    /// declarations can be used before the line that declares them.
    fn visit_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Some((name, span)) = declared_name(stmt) {
                self.declare(name, span);
            }
        }
        for stmt in stmts {
            self.visit_stmt(stmt);
        }
    }

    fn declaration(&self, span: Span) -> Declaration {
        (
            position_from_offset(self.source, span.start),
            position_from_offset(self.source, span.end),
        )
    }

    fn declare(&mut self, name: &str, span: Span) {
        let declaration = self.declaration(span);
        self.scope_tracker.declare(name.to_string(), declaration);
    }

    /// Marks every name in a type annotation (`Map<String, Money>?`) as used.
    fn use_type(&mut self, ty: &str) {
        for word in ty.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if !word.is_empty() {
                self.scope_tracker.use_var(word);
            }
        }
    }

    fn use_generics(&mut self, generics: &[GenericParam]) {
        for generic in generics {
            for constraint in &generic.constraints {
                self.use_type(constraint);
            }
        }
    }

    /// Declares parameters in the current scope. A function without a body
    /// (in an interface, or abstract) cannot use them, so they are not declared.
    fn visit_params(&mut self, params: &[Param], has_body: bool) {
        for param in params {
            if let Some(ty) = &param.ty {
                self.use_type(ty);
            }
            if let Some(def) = &param.default {
                self.check_assignment_spacing(param.name_span.end, def.span.start);
                self.visit_expr(def);
            }
            if has_body {
                self.declare(&param.name, param.name_span);
                if let Some(scope) = self.scope_tracker.scopes.last_mut() {
                    scope.params.push(param.name.clone());
                }
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Module {
                name,
                name_span,
                body,
                ..
            } => {
                self.declare(name, *name_span);
                // Other files use a module by its name.
                self.scope_tracker.use_var(name);
                if let Some(stmts) = body {
                    self.scope_tracker.enter_scope();
                    self.visit_stmts(stmts);
                    self.check_unused_in_current_scope();
                    self.scope_tracker.exit_scope();
                }
            }
            StmtKind::MacroDecl {
                name,
                name_span,
                body,
                ..
            } => {
                self.declare(name, *name_span);
                self.visit_stmt(body);
            }
            StmtKind::Let {
                name,
                name_span,
                ty,
                value,
                ..
            } => {
                if let Some(ty) = ty {
                    self.use_type(ty);
                }
                if let Some(v) = value {
                    self.check_assignment_spacing(name_span.end, v.span.start);
                    self.visit_expr(v);
                }
                // Check for shadowing before declaring
                if self.config.warn_shadowed_vars {
                    // Find where it was declared in outer scope
                    let mut related = vec![];
                    for scope in self.scope_tracker.scopes.iter().rev().skip(1) {
                        if let Some(((l, c), _)) = scope.declarations.get(name) {
                            related.push(LintRelatedInformation {
                                message: format!("Original declaration of '{}' is here", name),
                                line: *l,
                                column: *c,
                                file: None,
                            });
                            break;
                        }
                    }

                    if !related.is_empty() {
                        let mut diagnostic = diagnostic_at(
                            self.source,
                            "no-shadow",
                            format!("Variable '{}' shadows a variable from an outer scope", name),
                            Severity::Warning,
                            *name_span,
                        );
                        diagnostic.related_information = related;
                        self.diagnostics.push(diagnostic);
                    }
                }
                self.declare(name, *name_span);
            }
            StmtKind::LetPattern {
                pattern, ty, value, ..
            } => {
                if let Some(ty) = ty {
                    self.use_type(ty);
                }
                self.check_assignment_spacing(pattern.span.end, value.span.start);
                self.visit_expr(value);
                for (name, span) in pattern.bindings() {
                    self.declare(name, span);
                }
            }
            StmtKind::AssignProperty { object, value, .. } => {
                self.check_compound_assignment_spacing(stmt, value);
                self.visit_expr(object);
                self.visit_expr(value);
            }
            StmtKind::AssignIndex {
                collection,
                index,
                value,
            } => {
                self.check_compound_assignment_spacing(stmt, value);
                self.visit_expr(collection);
                self.visit_expr(index);
                self.visit_expr(value);
            }
            StmtKind::Assign { name, value } => {
                self.check_compound_assignment_spacing(stmt, value);
                self.visit_expr(value);
                self.scope_tracker.use_var(name);
            }
            StmtKind::Block(stmts) => {
                self.scope_tracker.enter_scope();
                self.visit_stmts(stmts);
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            StmtKind::Fn {
                name,
                name_span,
                generics,
                params,
                return_ty,
                body,
                ..
            } => {
                self.declare(name, *name_span);
                self.use_generics(generics);
                if let Some(ty) = return_ty {
                    self.use_type(ty);
                }

                self.scope_tracker.enter_scope();
                self.visit_params(params, has_body(body));
                self.visit_stmt(body);
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            StmtKind::Import { kind, .. } => {
                let span = stmt.span;
                match kind {
                    ImportKind::Module { alias } => {
                        if let Some(name) = alias {
                            let span = self.find_word(span, name);
                            self.declare(name, span);
                        }
                    }
                    ImportKind::Names(items) => {
                        for item in items {
                            let name = item.alias.as_ref().unwrap_or(&item.name);
                            let span = self.find_word(span, name);
                            self.declare(name, span);
                        }
                    }
                    ImportKind::Star { alias } => {
                        let span = self.find_word(span, alias);
                        self.declare(alias, span);
                    }
                }
            }
            StmtKind::CFor {
                init,
                condition,
                step,
                body,
            } => {
                self.scope_tracker.enter_scope();
                if let Some(i) = init {
                    self.visit_stmt(i);
                }
                if let Some(c) = condition {
                    self.visit_expr(c);
                }
                if let Some(s) = step {
                    self.visit_stmt(s);
                }
                self.visit_stmt(body);
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            StmtKind::DoWhile { body, condition } => {
                self.visit_stmt(body);
                self.visit_expr(condition);
            }
            StmtKind::Throw(expr) => self.visit_expr(expr),
            StmtKind::Labeled { stmt: inner, .. } => self.visit_stmt(inner),
            StmtKind::Export(inner) => {
                self.visit_stmt(inner);
                // Exported names are used by the modules that import them.
                if let Some((name, _)) = declared_name(inner) {
                    self.scope_tracker.use_var(name);
                }
                if let StmtKind::Let { name, .. } = &inner.kind {
                    self.scope_tracker.use_var(name);
                }
            }
            StmtKind::ExportList(names) => {
                for export in names {
                    self.scope_tracker.use_var(&export.name);
                }
            }
            StmtKind::TypeAlias {
                name,
                name_span,
                ty,
                ..
            } => {
                self.declare(name, *name_span);
                self.use_type(ty);
            }
            StmtKind::Extend {
                target,
                generics,
                methods,
                ..
            } => {
                self.use_type(target);
                self.use_generics(generics);
                for method in methods {
                    self.visit_fn_decl(method, true);
                }
            }
            StmtKind::Enum(enum_decl) => {
                self.declare(&enum_decl.name, enum_decl.name_span);
                // Variants are used through the enum (`Color.Red`) or in
                // patterns, so they are not reported as unused.
                for variant in &enum_decl.variants {
                    if let Some(params) = &variant.params {
                        for param in params {
                            if let Some(ty) = &param.ty {
                                self.use_type(ty);
                            }
                        }
                    }
                    if let Some(value) = &variant.value {
                        self.visit_expr(value);
                    }
                }
                for method in &enum_decl.methods {
                    self.visit_fn_decl(method, true);
                }
            }
            StmtKind::Interface {
                name,
                name_span,
                generics,
                methods,
                properties,
                extends,
                ..
            } => {
                self.declare(name, *name_span);
                self.use_generics(generics);
                for parent in extends {
                    self.use_type(parent);
                }
                for prop in properties {
                    self.visit_property(prop);
                }
                for method in methods {
                    self.visit_fn_decl(method, false);
                }
            }
            StmtKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.visit_expr(condition);
                self.visit_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.visit_stmt(eb);
                }
            }
            StmtKind::While { condition, body } => {
                self.visit_expr(condition);
                self.visit_stmt(body);
            }
            StmtKind::For {
                names,
                iterable,
                body,
                ..
            } => {
                self.visit_expr(iterable);
                self.scope_tracker.enter_scope();
                for name in names {
                    let span = self.find_word(stmt.span, name);
                    self.declare(name, span);
                }
                self.visit_stmt(body);
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            StmtKind::Return(Some(value)) => {
                self.visit_expr(value);
            }
            StmtKind::Return(None) => {}
            StmtKind::Expr(expr) => self.visit_expr(expr),
            StmtKind::Class {
                name,
                name_span,
                generics,
                methods,
                fields,
                properties,
                superclass,
                implements,
                nested_classes,
                ..
            } => {
                self.declare(name, *name_span);
                self.use_generics(generics);
                if let Some(parent) = superclass {
                    self.use_type(parent);
                }
                for interface in implements {
                    self.use_type(interface);
                }

                self.scope_tracker.enter_scope();
                for field in fields {
                    if let Some(ty) = &field.ty {
                        self.use_type(ty);
                    }
                    if let Some(def) = &field.default {
                        self.check_assignment_spacing(field.name_span.end, def.span.start);
                        self.visit_expr(def);
                    }
                }
                for prop in properties {
                    self.visit_property(prop);
                }
                for method in methods {
                    self.visit_fn_decl(method, !method.is_abstract);
                }
                for nested in nested_classes {
                    self.visit_stmt(nested);
                }
                self.scope_tracker.exit_scope();
            }
            StmtKind::Try {
                try_branch,
                catch_branches,
                finally_branch,
            } => {
                self.visit_stmt(try_branch);
                for catch in catch_branches {
                    if let Some(ty) = &catch.ty {
                        self.use_type(ty);
                    }
                    self.scope_tracker.enter_scope();
                    self.declare(&catch.name, catch.name_span);
                    self.visit_stmt(&catch.body);
                    self.check_unused_in_current_scope();
                    self.scope_tracker.exit_scope();
                }
                if let Some(finally) = finally_branch {
                    self.visit_stmt(finally);
                }
            }
            StmtKind::Break(_) | StmtKind::Continue(_) => {}
        }
    }

    fn visit_property(&mut self, prop: &PropertyDecl) {
        if let Some(ty) = &prop.ty {
            self.use_type(ty);
        }
        if let Some(getter) = &prop.getter {
            self.visit_fn_decl(getter, true);
        }
        if let Some(setter) = &prop.setter {
            self.visit_fn_decl(setter, true);
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Ident(name) => self.scope_tracker.use_var(name),
            ExprKind::Binary { left, right, .. } => {
                self.check_operator_spacing(left.span.end, right.span.start);
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::Unary { expr: inner, .. } => self.visit_expr(inner),
            ExprKind::Call { callee, args } | ExprKind::SafeCall { callee, args } => {
                self.visit_expr(callee);
                self.visit_args(args);
            }
            ExprKind::Property { object, .. } | ExprKind::SafeProperty { object, .. } => {
                self.visit_expr(object);
            }
            ExprKind::Block(stmts) => {
                self.scope_tracker.enter_scope();
                self.visit_stmts(stmts);
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            ExprKind::Lambda {
                params,
                body,
                return_ty,
                ..
            } => {
                if let Some(ty) = return_ty {
                    self.use_type(ty);
                }
                self.scope_tracker.enter_scope();
                self.visit_params(params, true);
                self.visit_expr(body);
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            ExprKind::Match { expr, arms } => {
                self.visit_expr(expr);
                for arm in arms {
                    self.scope_tracker.enter_scope();
                    self.visit_pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_expr(&arm.body);
                    self.check_unused_in_current_scope();
                    self.scope_tracker.exit_scope();
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.visit_expr(condition);
                self.visit_expr(then_branch);
                if let Some(eb) = else_branch {
                    self.visit_expr(eb);
                }
            }
            ExprKind::Literal(_) => {}
            ExprKind::ArrayLiteral(elements)
            | ExprKind::TupleLiteral(elements)
            | ExprKind::SetLiteral(elements) => {
                for e in elements {
                    self.visit_expr(e);
                }
            }
            ExprKind::InterpolatedString(parts) => {
                let outer = std::mem::replace(&mut self.in_interpolation, true);
                for part in parts {
                    self.visit_expr(part);
                }
                self.in_interpolation = outer;
            }
            ExprKind::MapLiteral(items) => {
                for item in items {
                    match item {
                        MapItem::Entry(_, value) => self.visit_expr(value),
                        MapItem::Spread(expr) => self.visit_expr(expr),
                    }
                }
            }
            ExprKind::NamedTupleLiteral(items) => {
                for (_, value) in items {
                    self.visit_expr(value);
                }
            }
            ExprKind::Range { start, end, .. } => {
                if let Some(s) = start {
                    self.visit_expr(s);
                }
                if let Some(e) = end {
                    self.visit_expr(e);
                }
            }
            ExprKind::Index { collection, index } | ExprKind::SafeIndex { collection, index } => {
                self.visit_expr(collection);
                self.visit_expr(index);
            }
            ExprKind::Slice {
                collection,
                start,
                end,
            } => {
                self.visit_expr(collection);
                if let Some(s) = start {
                    self.visit_expr(s);
                }
                if let Some(e) = end {
                    self.visit_expr(e);
                }
            }
            ExprKind::Pipeline { left, right } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::AssertNotNull(expr)
            | ExprKind::Spread(expr)
            | ExprKind::Paren(expr)
            | ExprKind::MacroSplice(expr) => {
                self.visit_expr(expr);
            }
            ExprKind::Await { expr } => {
                self.visit_expr(expr);
            }
            ExprKind::Is { left, ty } => {
                self.use_type(ty);
                self.visit_expr(left);
            }
            ExprKind::Specialize { left, type_args } => {
                for ty in type_args {
                    self.use_type(ty);
                }
                self.visit_expr(left);
            }
            ExprKind::MacroCall { name, args } => {
                self.scope_tracker.use_var(name);
                self.visit_args(args);
            }
            ExprKind::This | ExprKind::Super | ExprKind::Underscore => {}
        }
    }

    fn visit_args(&mut self, args: &[CallArg]) {
        for arg in args {
            match arg {
                CallArg::Positional(e) | CallArg::Named(_, e) | CallArg::Spread(e) => {
                    self.visit_expr(e);
                }
                CallArg::Placeholder => {}
            }
        }
    }

    fn visit_fn_decl(&mut self, decl: &FunctionDecl, can_have_body: bool) {
        self.use_generics(&decl.generics);
        if let Some(ty) = &decl.return_ty {
            self.use_type(ty);
        }
        self.scope_tracker.enter_scope();
        self.visit_params(&decl.params, can_have_body && has_body(&decl.body));
        self.visit_stmt(&decl.body);
        self.check_unused_in_current_scope();
        self.scope_tracker.exit_scope();
    }

    fn visit_pattern(&mut self, pattern: &Pattern) {
        match &pattern.kind {
            PatternKind::Ident(name) => {
                self.declare(name, pattern.span);
            }
            PatternKind::Type(ty, name) => {
                self.use_type(ty);
                if let Some(name) = name {
                    let span = self.find_word(pattern.span, name);
                    self.declare(name, span);
                }
            }
            PatternKind::Variant(name, patterns) => {
                self.use_type(name);
                for p in patterns {
                    self.visit_pattern(p);
                }
            }
            PatternKind::Tuple(patterns) | PatternKind::Or(patterns) => {
                for p in patterns {
                    self.visit_pattern(p);
                }
            }
            PatternKind::Object(props, rest) => {
                for (_, p) in props {
                    self.visit_pattern(p);
                }
                if let Some(p) = rest {
                    self.visit_pattern(p);
                }
            }
            PatternKind::Rest(Some(name)) => {
                let span = self.find_word(pattern.span, name);
                self.declare(name, span);
            }
            _ => {}
        }
    }

    /// The span of `name` as a whole word inside `within`, or `within` itself.
    fn find_word(&self, within: Span, name: &str) -> Span {
        let end = within.end.min(self.source.len());
        let Some(text) = self.source.get(within.start..end) else {
            return within;
        };
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut from = 0;
        while let Some(pos) = text[from..].find(name) {
            let start = from + pos;
            let stop = start + name.len();
            let before = text[..start].chars().next_back();
            let after = text[stop..].chars().next();
            if !before.is_some_and(is_word) && !after.is_some_and(is_word) {
                return Span::new(within.start + start, within.start + stop);
            }
            from = stop;
        }
        within
    }

    /// A binary operator needs a space on each side: `a + b`, not `a+b`. The
    /// operator is whatever lies between its two operands in the source.
    fn check_operator_spacing(&mut self, left_end: usize, right_start: usize) {
        if !self.config.warn_operator_spacing || self.in_interpolation {
            return;
        }
        let Some(gap) = self.source.get(left_end..right_start) else {
            return;
        };
        let op = gap.trim();
        // Skip a gap that holds more than the operator: a parenthesis that is
        // not part of the operand's span, or a comment.
        if op.is_empty() || op.contains(['(', ')']) || op.contains("//") || op.contains("/*") {
            return;
        }
        let spaced = gap.starts_with(char::is_whitespace) && gap.ends_with(char::is_whitespace);
        if !spaced {
            let start = left_end + (gap.len() - gap.trim_start().len());
            self.push_operator_spacing(op, start);
        }
    }

    /// `=` (or `+=` and the other compound operators) before a value needs a
    /// space on each side. `region_start` is where the text before it starts,
    /// such as the end of the declared name.
    fn check_assignment_spacing(&mut self, region_start: usize, value_start: usize) {
        if !self.config.warn_operator_spacing
            || self.in_interpolation
            || region_start >= value_start
        {
            return;
        }
        let Some(text) = self.source.get(region_start..value_start) else {
            return;
        };
        let before_value = text.trim_end();
        let op_len = before_value
            .chars()
            .rev()
            .take_while(|c| "=+-*/%".contains(*c))
            .count();
        if op_len == 0 || !before_value.ends_with('=') {
            return;
        }
        let op_start = before_value.len() - op_len;
        let op = &before_value[op_start..];
        let space_after = before_value.len() < text.len();
        let space_before = op_start > 0 && before_value[..op_start].ends_with(char::is_whitespace);
        if !space_after || !space_before {
            self.push_operator_spacing(op, region_start + op_start);
        }
    }

    /// `x = v`, `obj.f = v` and `xs[i] = v`. A compound assignment (`x += 1`)
    /// becomes `x = x + 1`, whose value starts at the target, and is checked
    /// as that binary operator.
    fn check_compound_assignment_spacing(&mut self, stmt: &Stmt, value: &Expr) {
        if value.span.start > stmt.span.start {
            self.check_assignment_spacing(stmt.span.start, value.span.start);
        }
    }

    fn push_operator_spacing(&mut self, op: &str, start: usize) {
        self.diagnostics.push(diagnostic_at(
            self.source,
            "operator-spacing",
            format!("Missing spacing around operator '{}'", op),
            Severity::Info,
            Span::new(start, start + op.len()),
        ));
    }

    fn check_unused_in_current_scope(&mut self) {
        if !self.config.warn_unused_vars {
            return;
        }
        if let Some(scope) = self.scope_tracker.scopes.last() {
            // A parameter before a used one cannot be removed, so only
            // parameters after the last used one are reported.
            let last_used_param = scope
                .params
                .iter()
                .rposition(|param| scope.usages.contains(param));
            let kept_params: HashSet<&String> = match last_used_param {
                Some(last) => scope.params[..last].iter().collect(),
                None => HashSet::new(),
            };
            for (name, ((line, col), (end_line, end_col))) in &scope.declarations {
                if !scope.usages.contains(name)
                    && !name.starts_with('_')
                    && !kept_params.contains(name)
                {
                    self.diagnostics.push(LintDiagnostic {
                        rule_name: "no-unused-vars".to_string(),
                        message: format!("Variable '{}' is declared but never used", name),
                        severity: Severity::Warning,
                        line: *line,
                        column: *col,
                        end_line: *end_line,
                        end_column: (*end_col).max(*col + 1),
                        related_information: vec![],
                    });
                }
            }
        }
    }
}

/// The name a declaration statement introduces in its scope, for hoisting.
fn declared_name(stmt: &Stmt) -> Option<(&str, Span)> {
    match &stmt.kind {
        StmtKind::Fn {
            name, name_span, ..
        }
        | StmtKind::Class {
            name, name_span, ..
        }
        | StmtKind::Interface {
            name, name_span, ..
        }
        | StmtKind::TypeAlias {
            name, name_span, ..
        }
        | StmtKind::Module {
            name, name_span, ..
        }
        | StmtKind::MacroDecl {
            name, name_span, ..
        } => Some((name.as_str(), *name_span)),
        StmtKind::Enum(decl) => Some((decl.name.as_str(), decl.name_span)),
        StmtKind::Export(inner) => declared_name(inner),
        _ => None,
    }
}

/// Whether a function body has any statements (interface methods and
/// abstract methods have none).
fn has_body(body: &Stmt) -> bool {
    !matches!(&body.kind, StmtKind::Block(stmts) if stmts.is_empty())
}

pub fn lint(source: &str) -> Vec<LintDiagnostic> {
    lint_with_config(source, &LintConfig::default())
}

pub fn lint_with_config(source: &str, config: &LintConfig) -> Vec<LintDiagnostic> {
    let linter = Linter::with_config(config.clone());
    linter.lint(source)
}

pub fn format_diagnostics(diagnostics: &[LintDiagnostic]) -> String {
    let mut result = String::new();
    for diag in diagnostics {
        result.push_str(&format!(
            "[{}] {}: {} ({}:{})\n",
            diag.severity, diag.rule_name, diag.message, diag.line, diag.column
        ));
    }
    result
}

/// The 1-based line and column of a byte offset, counting UTF-16 code units.
fn position_from_offset(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += ch.len_utf16();
        }
    }
    (line, column)
}
