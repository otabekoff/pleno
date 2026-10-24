use pleno_ast::*;
use pleno_lexer::lex;
use pleno_parser::Parser;
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub rule_name: String,
    pub message: String,
    pub severity: Severity,
    pub line: usize,
    pub column: usize,
    pub related_information: Vec<LintRelatedInformation>,
}

#[derive(Debug, Clone)]
pub struct LintRulesConfig {
    pub warn_unused_vars: bool,
    pub max_line_length: Option<usize>,
    pub warn_trailing_spaces: bool,
    pub warn_semi_spacing: bool,
    pub prefer_const: bool,
    pub no_undef: bool,
    pub warn_shadowed_vars: bool,
}

impl Default for LintRulesConfig {
    fn default() -> Self {
        Self {
            warn_unused_vars: true,
            max_line_length: Some(80),
            warn_trailing_spaces: true,
            warn_semi_spacing: true,
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

struct Scope {
    declarations: HashMap<String, (usize, usize)>,
    usages: HashSet<String>,
}

impl Scope {
    fn new() -> Self {
        Self {
            declarations: HashMap::new(),
            usages: HashSet::new(),
        }
    }
}

struct ScopeTracker {
    scopes: Vec<Scope>,
    globals: HashSet<String>,
}

impl ScopeTracker {
    fn new() -> Self {
        let mut globals = HashSet::new();
        let builtins = [
            "print", "println", "id", "typeof", "map", "Int", "Float", "String", "Array", "Map",
            "JSON", "Math", "OS", "Promise", "Channel", "Mutex", "spawn", "await",
        ];
        for b in builtins {
            globals.insert(b.to_string());
        }

        Self {
            scopes: vec![Scope::new()],
            globals,
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn exit_scope(&mut self) -> Scope {
        self.scopes.pop().expect("Exited root scope")
    }

    fn declare(&mut self, name: String, line: usize, column: usize) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.declarations.insert(name, (line, column));
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

    fn is_declared(&self, name: &str) -> bool {
        if self.globals.contains(name) {
            return true;
        }
        for scope in self.scopes.iter().rev() {
            if scope.declarations.contains_key(name) {
                return true;
            }
        }
        false
    }
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
                let (line, column) = position_from_offset(source, e.span.start);
                diagnostics.push(LintDiagnostic {
                    rule_name: "syntax-error".to_string(),
                    message: e.message,
                    severity: Severity::Error,
                    line,
                    column,
                    related_information: vec![],
                });
                return diagnostics;
            }
        };

        let program = match Parser::new(tokens, source).parse_program() {
            Ok(program) => program,
            Err(e) => {
                let (line, column) = position_from_offset(source, e.span.start);
                diagnostics.push(LintDiagnostic {
                    rule_name: "parse-error".to_string(),
                    message: e.message,
                    severity: Severity::Error,
                    line,
                    column,
                    related_information: vec![],
                });
                return diagnostics;
            }
        };

        let mut visitor = LintVisitor {
            source,
            scope_tracker: ScopeTracker::new(),
            diagnostics: Vec::new(),
            config: &self.config.rules,
        };

        visitor.visit_program(&program);
        diagnostics.extend(visitor.diagnostics);

        if self.config.rules.max_line_length.is_some() {
            diagnostics.extend(self.check_max_line_length(source));
        }
        if self.config.rules.warn_trailing_spaces {
            diagnostics.extend(self.check_trailing_spaces(source));
        }
        if self.config.rules.warn_semi_spacing {
            diagnostics.extend(self.check_semi_spacing(source));
        }

        diagnostics.extend(self.check_operator_spacing(source));

        diagnostics
    }

    fn check_max_line_length(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        let max_length = self.config.rules.max_line_length.unwrap_or(80);

        for (line_num, line) in source.lines().enumerate() {
            if line.len() > max_length {
                diagnostics.push(LintDiagnostic {
                    rule_name: "max-line-length".to_string(),
                    message: format!(
                        "Line exceeds maximum line length of {} characters",
                        max_length
                    ),
                    severity: Severity::Warning,
                    line: line_num + 1,
                    column: max_length + 1,
                    related_information: vec![],
                });
            }
        }
        diagnostics
    }

    fn check_trailing_spaces(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        for (line_num, line) in source.lines().enumerate() {
            if line.ends_with(' ') || line.ends_with('\t') {
                diagnostics.push(LintDiagnostic {
                    rule_name: "no-trailing-spaces".to_string(),
                    message: "Line has trailing spaces".to_string(),
                    severity: Severity::Info,
                    line: line_num + 1,
                    column: line.len(),
                    related_information: vec![],
                });
            }
        }
        diagnostics
    }

    fn check_semi_spacing(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        for (line_num, line) in source.lines().enumerate() {
            if let Some(semi_pos) = line.find(';') {
                let after_semi = &line[semi_pos + 1..];
                if !after_semi.is_empty()
                    && !after_semi.starts_with(' ')
                    && !after_semi.trim_start().is_empty()
                {
                    diagnostics.push(LintDiagnostic {
                        rule_name: "semi-spacing".to_string(),
                        message: "Expected space after semicolon".to_string(),
                        severity: Severity::Info,
                        line: line_num + 1,
                        column: semi_pos + 2,
                        related_information: vec![],
                    });
                }
            }
        }
        diagnostics
    }

    fn check_operator_spacing(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        let ops = ["+", "-", "*", "/", "=", "==", ">", "<", ">=", "<="];

        for (line_num, line) in source.lines().enumerate() {
            // Very simple check to avoid strings/comments for now
            if line.trim().starts_with("//")
                || line.trim().starts_with("/*")
                || line.contains('"')
                || line.contains('\'')
            {
                continue;
            }

            for op in &ops {
                let mut last_pos = 0;
                while let Some(pos) = line[last_pos..].find(op) {
                    let abs_pos = last_pos + pos;

                    // Avoid composite op confusion
                    let is_part = if *op == "=" {
                        (abs_pos > 0
                            && matches!(
                                line.as_bytes()[abs_pos - 1],
                                b'=' | b'>' | b'<' | b'!' | b'+' | b'-' | b'*' | b'/'
                            ))
                            || (abs_pos + 1 < line.len() && line.as_bytes()[abs_pos + 1] == b'=')
                    } else if *op == ">" || *op == "<" || *op == "+" || *op == "-" {
                        (abs_pos + 1 < line.len()
                            && (line.as_bytes()[abs_pos + 1] == b'='
                                || line.as_bytes()[abs_pos + 1] == line.as_bytes()[abs_pos]))
                            || (abs_pos > 0
                                && line.as_bytes()[abs_pos - 1] == line.as_bytes()[abs_pos])
                    } else {
                        false
                    };

                    if !is_part {
                        let space_before = abs_pos == 0
                            || line.as_bytes()[abs_pos - 1] == b' '
                            || line.as_bytes()[abs_pos - 1] == b'('
                            || line.as_bytes()[abs_pos - 1] == b'[';
                        let space_after = abs_pos + op.len() >= line.len()
                            || line.as_bytes()[abs_pos + op.len()] == b' '
                            || line.as_bytes()[abs_pos + op.len()] == b')'
                            || line.as_bytes()[abs_pos + op.len()] == b']';

                        if !space_before || !space_after {
                            diagnostics.push(LintDiagnostic {
                                rule_name: "operator-spacing".to_string(),
                                message: format!("Missing spacing around operator '{}'", op),
                                severity: Severity::Warning,
                                line: line_num + 1,
                                column: abs_pos + 1,
                                related_information: vec![],
                            });
                            break;
                        }
                    }
                    last_pos = abs_pos + op.len();
                }
            }
        }
        diagnostics
    }
}

struct LintVisitor<'a> {
    source: &'a str,
    scope_tracker: ScopeTracker,
    diagnostics: Vec<LintDiagnostic>,
    config: &'a LintRulesConfig,
}

impl<'a> LintVisitor<'a> {
    fn visit_program(&mut self, program: &Program) {
        for stmt in &program.statements {
            self.visit_stmt(stmt);
        }
        self.check_unused_in_current_scope();
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Module {
                name,
                name_span,
                body,
                ..
            } => {
                let (line, col) = position_from_offset(self.source, name_span.start);
                self.scope_tracker.declare(name.clone(), line, col);
                if let Some(stmts) = body {
                    self.scope_tracker.enter_scope();
                    for s in stmts {
                        self.visit_stmt(s);
                    }
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
                let (line, col) = position_from_offset(self.source, name_span.start);
                self.scope_tracker.declare(name.clone(), line, col);
                self.visit_stmt(body);
            }
            StmtKind::Let {
                name,
                name_span,
                value,
                ..
            } => {
                if let Some(v) = value {
                    self.visit_expr(v);
                }
                let (line, col) = position_from_offset(self.source, name_span.start);
                // Check for shadowing before declaring
                if self.config.warn_shadowed_vars {
                    // Find where it was declared in outer scope
                    let mut related = vec![];
                    for scope in self.scope_tracker.scopes.iter().rev().skip(1) {
                        if let Some((l, c)) = scope.declarations.get(name) {
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
                        self.diagnostics.push(LintDiagnostic {
                            rule_name: "no-shadow".to_string(),
                            message: format!(
                                "Variable '{}' shadows a variable from an outer scope",
                                name
                            ),
                            severity: Severity::Warning,
                            line,
                            column: col,
                            related_information: related,
                        });
                    }
                }
                self.scope_tracker.declare(name.clone(), line, col);
            }
            StmtKind::AssignProperty { object, value, .. } => {
                self.visit_expr(object);
                self.visit_expr(value);
            }
            StmtKind::AssignIndex {
                collection,
                index,
                value,
            } => {
                self.visit_expr(collection);
                self.visit_expr(index);
                self.visit_expr(value);
            }
            StmtKind::Assign { name, value } => {
                self.visit_expr(value);
                if self.config.no_undef && !self.scope_tracker.is_declared(name) {
                    let (line, col) = position_from_offset(self.source, stmt.span.start);
                    self.diagnostics.push(LintDiagnostic {
                        rule_name: "no-undef".to_string(),
                        message: format!("Usage of undefined variable '{}'", name),
                        severity: Severity::Error,
                        line,
                        column: col,
                        related_information: vec![],
                    });
                } else {
                    self.scope_tracker.use_var(name);
                }
            }
            StmtKind::Block(stmts) => {
                self.scope_tracker.enter_scope();
                for s in stmts {
                    self.visit_stmt(s);
                }
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            StmtKind::Fn {
                name,
                name_span,
                params,
                body,
                ..
            } => {
                let (line, col) = position_from_offset(self.source, name_span.start);
                self.scope_tracker.declare(name.clone(), line, col);

                self.scope_tracker.enter_scope();
                for param in params {
                    let (pl, pc) = position_from_offset(self.source, param.name_span.start);
                    self.scope_tracker.declare(param.name.clone(), pl, pc);
                    if let Some(def) = &param.default {
                        self.visit_expr(def);
                    }
                }
                self.visit_stmt(body);
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            StmtKind::Import { kind, .. } => {
                let (line, col) = position_from_offset(self.source, stmt.span.start);
                match kind {
                    ImportKind::Module { alias } => {
                        if let Some(name) = alias {
                            self.scope_tracker.declare(name.clone(), line, col);
                        }
                    }
                    ImportKind::Names(items) => {
                        for item in items {
                            let name = item.alias.as_ref().unwrap_or(&item.name);
                            self.scope_tracker.declare(name.clone(), line, col);
                        }
                    }
                    ImportKind::Star { alias } => {
                        self.scope_tracker.declare(alias.clone(), line, col);
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
            StmtKind::Export(inner) => self.visit_stmt(inner),
            StmtKind::TypeAlias {
                name, name_span, ..
            } => {
                let (line, col) = position_from_offset(self.source, name_span.start);
                self.scope_tracker.declare(name.clone(), line, col);
            }
            StmtKind::Extend { methods, .. } => {
                for method in methods {
                    self.visit_fn_decl(method);
                }
            }
            StmtKind::Enum(enum_decl) => {
                let (line, col) = position_from_offset(self.source, enum_decl.name_span.start);
                self.scope_tracker
                    .declare(enum_decl.name.clone(), line, col);

                self.scope_tracker.enter_scope();
                for variant in &enum_decl.variants {
                    let (vl, vc) = position_from_offset(self.source, variant.name_span.start);
                    self.scope_tracker.declare(variant.name.clone(), vl, vc);
                }
                for method in &enum_decl.methods {
                    self.visit_fn_decl(method);
                }
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            StmtKind::Interface {
                name,
                name_span,
                methods,
                properties,
                ..
            } => {
                let (line, col) = position_from_offset(self.source, name_span.start);
                self.scope_tracker.declare(name.clone(), line, col);

                self.scope_tracker.enter_scope();
                for prop in properties {
                    if let Some(getter) = &prop.getter {
                        self.visit_fn_decl(getter);
                    }
                    if let Some(setter) = &prop.setter {
                        self.visit_fn_decl(setter);
                    }
                }
                for method in methods {
                    self.visit_fn_decl(method);
                }
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
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
                let (line, col) = position_from_offset(self.source, stmt.span.start);
                for name in names {
                    self.scope_tracker.declare(name.clone(), line, col);
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
                methods,
                fields,
                ..
            } => {
                let (line, col) = position_from_offset(self.source, name_span.start);
                self.scope_tracker.declare(name.clone(), line, col);

                self.scope_tracker.enter_scope();
                // Inside class, 'this' is available
                self.scope_tracker.globals.insert("this".to_string());
                self.scope_tracker.globals.insert("super".to_string());

                for field in fields {
                    if let Some(def) = &field.default {
                        self.visit_expr(def);
                    }
                }
                for method in methods {
                    self.visit_fn_decl(method);
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
                    self.scope_tracker.enter_scope();
                    let (line, col) = position_from_offset(self.source, stmt.span.start);
                    self.scope_tracker.declare(catch.name.clone(), line, col);
                    self.visit_stmt(&catch.body);
                    self.check_unused_in_current_scope();
                    self.scope_tracker.exit_scope();
                }
                if let Some(finally) = finally_branch {
                    self.visit_stmt(finally);
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Ident(name) => {
                if self.config.no_undef && !self.scope_tracker.is_declared(name) {
                    let (line, col) = position_from_offset(self.source, expr.span.start);
                    self.diagnostics.push(LintDiagnostic {
                        rule_name: "no-undef".to_string(),
                        message: format!("Usage of undefined variable '{}'", name),
                        severity: Severity::Error,
                        line,
                        column: col,
                        related_information: vec![],
                    });
                } else {
                    self.scope_tracker.use_var(name);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            ExprKind::Unary { expr: inner, .. } => self.visit_expr(inner),
            ExprKind::Call { callee, args } | ExprKind::SafeCall { callee, args } => {
                self.visit_expr(callee);
                for arg in args {
                    match arg {
                        CallArg::Positional(e) | CallArg::Named(_, e) | CallArg::Spread(e) => {
                            self.visit_expr(e);
                        }
                        CallArg::Placeholder => {}
                    }
                }
            }
            ExprKind::Property { object, .. } | ExprKind::SafeProperty { object, .. } => {
                self.visit_expr(object);
            }
            ExprKind::Block(stmts) => {
                self.scope_tracker.enter_scope();
                for s in stmts {
                    self.visit_stmt(s);
                }
                self.check_unused_in_current_scope();
                self.scope_tracker.exit_scope();
            }
            ExprKind::Lambda { params, body, .. } => {
                self.scope_tracker.enter_scope();
                for param in params {
                    let (pl, pc) = position_from_offset(self.source, expr.span.start);
                    self.scope_tracker.declare(param.name.clone(), pl, pc);
                    if let Some(def) = &param.default {
                        self.visit_expr(def);
                    }
                }
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
            ExprKind::Is { left, .. } => {
                self.visit_expr(left);
            }
            ExprKind::Specialize { left, .. } => {
                self.visit_expr(left);
            }
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    self.visit_expr(part);
                }
            }
            ExprKind::MacroCall { args, .. } => {
                for arg in args {
                    match arg {
                        CallArg::Positional(e) | CallArg::Named(_, e) | CallArg::Spread(e) => {
                            self.visit_expr(e);
                        }
                        CallArg::Placeholder => {}
                    }
                }
            }
            ExprKind::This | ExprKind::Super | ExprKind::Underscore => {}
        }
    }

    fn visit_fn_decl(&mut self, decl: &FunctionDecl) {
        self.scope_tracker.enter_scope();
        for param in &decl.params {
            let (pl, pc) = position_from_offset(self.source, param.name_span.start);
            self.scope_tracker.declare(param.name.clone(), pl, pc);
            if let Some(def) = &param.default {
                self.visit_expr(def);
            }
        }
        self.visit_stmt(&decl.body);
        self.check_unused_in_current_scope();
        self.scope_tracker.exit_scope();
    }

    fn visit_pattern(&mut self, pattern: &Pattern) {
        match &pattern.kind {
            PatternKind::Ident(name) => {
                let (line, col) = position_from_offset(self.source, pattern.span.start);
                self.scope_tracker.declare(name.clone(), line, col);
            }
            PatternKind::Type(_, Some(name)) => {
                let (line, col) = position_from_offset(self.source, pattern.span.start);
                self.scope_tracker.declare(name.clone(), line, col);
            }
            PatternKind::Tuple(patterns)
            | PatternKind::Variant(_, patterns)
            | PatternKind::Or(patterns) => {
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
                let (line, col) = position_from_offset(self.source, pattern.span.start);
                self.scope_tracker.declare(name.clone(), line, col);
            }
            _ => {}
        }
    }

    fn check_unused_in_current_scope(&mut self) {
        if !self.config.warn_unused_vars {
            return;
        }
        if let Some(scope) = self.scope_tracker.scopes.last() {
            for (name, (line, col)) in &scope.declarations {
                if !scope.usages.contains(name) && !name.starts_with('_') {
                    self.diagnostics.push(LintDiagnostic {
                        rule_name: "no-unused-vars".to_string(),
                        message: format!("Variable '{}' is declared but never used", name),
                        severity: Severity::Warning,
                        line: *line,
                        column: *col,
                        related_information: vec![],
                    });
                }
            }
        }
    }
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
            column += 1;
        }
    }
    (line, column)
}
