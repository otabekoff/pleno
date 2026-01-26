use pleno_ast::*;
use pleno_lexer::lex;
use pleno_parser::Parser;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct LintRule {
    pub name: String,
    pub description: String,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    pub rule_name: String,
    pub message: String,
    pub severity: Severity,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct LintRulesConfig {
    pub warn_unused_vars: bool,
    pub max_line_length: Option<usize>,
    pub warn_trailing_spaces: bool,
    pub warn_semi_spacing: bool,
    pub prefer_const: bool,
}

impl Default for LintRulesConfig {
    fn default() -> Self {
        Self {
            warn_unused_vars: true,
            max_line_length: Some(80),
            warn_trailing_spaces: true,
            warn_semi_spacing: true,
            prefer_const: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    pub rules: LintRulesConfig,
}

pub struct Linter {
    rules: Vec<LintRule>,
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
        let rules = vec![
            LintRule {
                name: "no-unused-vars".to_string(),
                description: "Detects unused variables".to_string(),
                severity: Severity::Warning,
            },
            LintRule {
                name: "no-console-log".to_string(),
                description: "Disallows console.log statements".to_string(),
                severity: Severity::Warning,
            },
            LintRule {
                name: "prefer-const".to_string(),
                description: "Recommends using const instead of let when possible".to_string(),
                severity: Severity::Info,
            },
            LintRule {
                name: "max-line-length".to_string(),
                description: "Ensures lines don't exceed max length".to_string(),
                severity: Severity::Warning,
            },
            LintRule {
                name: "no-trailing-spaces".to_string(),
                description: "Removes trailing spaces".to_string(),
                severity: Severity::Info,
            },
            LintRule {
                name: "semi-spacing".to_string(),
                description: "Enforces consistent semicolon spacing".to_string(),
                severity: Severity::Info,
            },
            LintRule {
                name: "brace-style".to_string(),
                description: "Enforces consistent brace style".to_string(),
                severity: Severity::Info,
            },
        ];

        Self { rules, config }
    }

    pub fn lint(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();

        // First, tokenize the source
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
                });
                return diagnostics;
            }
        };

        // Then, parse it to get the AST
        let ast = match Parser::new(tokens).parse_program() {
            Ok(program) => program,
            Err(e) => {
                let (line, column) = position_from_offset(source, e.span.start);
                diagnostics.push(LintDiagnostic {
                    rule_name: "parse-error".to_string(),
                    message: e.message,
                    severity: Severity::Error,
                    line,
                    column,
                });
                return diagnostics;
            }
        };

        // Apply lint rules to the AST based on config
        if self.config.rules.warn_unused_vars {
            diagnostics.extend(self.check_unused_variables(source, &ast));
        }
        if self.config.rules.max_line_length.is_some() {
            diagnostics.extend(self.check_max_line_length(source));
        }
        if self.config.rules.warn_trailing_spaces {
            diagnostics.extend(self.check_trailing_spaces(source));
        }
        if self.config.rules.warn_semi_spacing {
            diagnostics.extend(self.check_semi_spacing(source));
        }

        // Check for operator spacing
        diagnostics.extend(self.check_operator_spacing(source));

        diagnostics
    }

    fn check_unused_variables(&self, source: &str, program: &Program) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        let mut declarations: Vec<(String, usize, usize)> = Vec::new();
        let mut usages: HashSet<String> = HashSet::new();

        // Collect all variable declarations
        for stmt in &program.statements {
            collect_declarations(stmt, source, &mut declarations);
        }

        // Collect all variable usages
        for stmt in &program.statements {
            collect_usages_from_stmt(stmt, &mut usages);
        }

        // Report unused variables (skip _ prefixed)
        for (name, line, column) in declarations {
            if !usages.contains(&name) && !name.starts_with('_') {
                diagnostics.push(LintDiagnostic {
                    rule_name: "no-unused-vars".to_string(),
                    message: format!("Variable '{}' is declared but never used", name),
                    severity: Severity::Warning,
                    line,
                    column,
                });
            }
        }

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
                        "Line {} exceeds maximum line length of {} characters",
                        line_num + 1,
                        max_length
                    ),
                    severity: Severity::Warning,
                    line: line_num + 1,
                    column: max_length + 1,
                });
            }
        }

        diagnostics
    }

    fn check_operator_spacing(&self, source: &str) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();
        let ops = ["+", "-", "*", "/", "=", "==", ">", "<", ">=", "<="];

        for (line_num, line) in source.lines().enumerate() {
            for op in &ops {
                let mut last_pos = 0;
                while let Some(pos) = line[last_pos..].find(op) {
                    let absolute_pos = last_pos + pos;

                    let is_part_of_composite = if *op == "=" {
                        (absolute_pos > 0
                            && (line.as_bytes()[absolute_pos - 1] == b'='
                                || line.as_bytes()[absolute_pos - 1] == b'>'
                                || line.as_bytes()[absolute_pos - 1] == b'<'))
                            || (absolute_pos + 1 < line.len()
                                && line.as_bytes()[absolute_pos + 1] == b'=')
                    } else if *op == ">" || *op == "<" {
                        absolute_pos + 1 < line.len() && line.as_bytes()[absolute_pos + 1] == b'='
                    } else {
                        false
                    };

                    if !is_part_of_composite {
                        let space_before =
                            absolute_pos == 0 || line.as_bytes()[absolute_pos - 1] == b' ';
                        let space_after = absolute_pos + op.len() >= line.len()
                            || line.as_bytes()[absolute_pos + op.len()] == b' ';

                        if !space_before || !space_after {
                            diagnostics.push(LintDiagnostic {
                                rule_name: "operator-spacing".to_string(),
                                message: format!(
                                    "Missing spacing around operator '{}' on line {}",
                                    op,
                                    line_num + 1
                                ),
                                severity: Severity::Warning,
                                line: line_num + 1,
                                column: absolute_pos + 1,
                            });
                            break;
                        }
                    }
                    last_pos = absolute_pos + op.len();
                }
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
                    message: format!("Line {} has trailing spaces", line_num + 1),
                    severity: Severity::Info,
                    line: line_num + 1,
                    column: line.len(),
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
                        message: format!("Expected space after semicolon on line {}", line_num + 1),
                        severity: Severity::Info,
                        line: line_num + 1,
                        column: semi_pos + 1,
                    });
                }
            }
        }

        diagnostics
    }

    pub fn add_rule(&mut self, rule: LintRule) {
        self.rules.push(rule);
    }

    pub fn run_rules(&self, source: &str, rule_names: &[&str]) -> Vec<LintDiagnostic> {
        let mut all_diagnostics = self.lint(source);
        all_diagnostics.retain(|diag| rule_names.contains(&diag.rule_name.as_str()));
        all_diagnostics
    }
}

// Helper functions for unused variable detection

fn collect_declarations(stmt: &Stmt, source: &str, declarations: &mut Vec<(String, usize, usize)>) {
    match &stmt.kind {
        StmtKind::Let { name, .. } => {
            let (line, column) = position_from_offset(source, stmt.span.start);
            declarations.push((name.clone(), line, column));
        }
        StmtKind::Block(stmts) => {
            for s in stmts {
                collect_declarations(s, source, declarations);
            }
        }
        StmtKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_declarations(then_branch, source, declarations);
            if let Some(eb) = else_branch {
                collect_declarations(eb, source, declarations);
            }
        }
        StmtKind::While { body, .. } => collect_declarations(body, source, declarations),
        StmtKind::For { body, .. } => collect_declarations(body, source, declarations),
        StmtKind::Fn { body, .. } => {
            collect_declarations(body, source, declarations);
        }
        StmtKind::Try {
            try_branch,
            catch_branches,
            finally_branch,
        } => {
            collect_declarations(try_branch, source, declarations);
            for cb in catch_branches {
                collect_declarations(&cb.body, source, declarations);
            }
            if let Some(fb) = finally_branch {
                collect_declarations(fb, source, declarations);
            }
        }
        _ => {}
    }
}

fn collect_usages_from_stmt(stmt: &Stmt, usages: &mut HashSet<String>) {
    match &stmt.kind {
        StmtKind::Expr(expr) => collect_usages_from_expr(expr, usages),
        StmtKind::Let { value: Some(v), .. } => {
            collect_usages_from_expr(v, usages);
        }
        StmtKind::Assign { value, .. } => {
            collect_usages_from_expr(value, usages);
        }
        StmtKind::Return(Some(expr)) => collect_usages_from_expr(expr, usages),
        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_usages_from_expr(condition, usages);
            collect_usages_from_stmt(then_branch, usages);
            if let Some(eb) = else_branch {
                collect_usages_from_stmt(eb, usages);
            }
        }
        StmtKind::While { condition, body } => {
            collect_usages_from_expr(condition, usages);
            collect_usages_from_stmt(body, usages);
        }
        StmtKind::For { iterable, body, .. } => {
            collect_usages_from_expr(iterable, usages);
            collect_usages_from_stmt(body, usages);
        }
        StmtKind::Block(stmts) => {
            for s in stmts {
                collect_usages_from_stmt(s, usages);
            }
        }
        StmtKind::Fn { body, .. } => {
            collect_usages_from_stmt(body, usages);
        }
        StmtKind::Throw(expr) => collect_usages_from_expr(expr, usages),
        _ => {}
    }
}

fn collect_usages_from_expr(expr: &Expr, usages: &mut HashSet<String>) {
    match &expr.kind {
        ExprKind::Ident(name) => {
            usages.insert(name.clone());
        }
        ExprKind::Binary { left, right, .. } => {
            collect_usages_from_expr(left, usages);
            collect_usages_from_expr(right, usages);
        }
        ExprKind::Unary { expr: inner, .. } => {
            collect_usages_from_expr(inner, usages);
        }
        ExprKind::Call { callee, args } => {
            collect_usages_from_expr(callee, usages);
            for arg in args {
                match arg {
                    CallArg::Positional(e) | CallArg::Named(_, e) => {
                        collect_usages_from_expr(e, usages);
                    }
                    CallArg::Placeholder => {}
                    CallArg::Spread(expr) => {
                        collect_usages_from_expr(expr, usages);
                    }
                }
            }
        }
        ExprKind::Property { object, .. } | ExprKind::SafeProperty { object, .. } => {
            collect_usages_from_expr(object, usages);
        }
        ExprKind::Index { collection, index } => {
            collect_usages_from_expr(collection, usages);
            collect_usages_from_expr(index, usages);
        }
        ExprKind::ArrayLiteral(items)
        | ExprKind::TupleLiteral(items)
        | ExprKind::SetLiteral(items) => {
            for item in items {
                collect_usages_from_expr(item, usages);
            }
        }
        ExprKind::MapLiteral(entries) => {
            for entry in entries {
                match entry {
                    MapItem::Entry(_, value) => collect_usages_from_expr(value, usages),
                    MapItem::Spread(e) => collect_usages_from_expr(e, usages),
                }
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_usages_from_expr(body, usages);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_usages_from_expr(condition, usages);
            collect_usages_from_expr(then_branch, usages);
            collect_usages_from_expr(else_branch, usages);
        }
        ExprKind::Pipeline { left, right } => {
            collect_usages_from_expr(left, usages);
            collect_usages_from_expr(right, usages);
        }
        ExprKind::Await { expr: inner } => {
            collect_usages_from_expr(inner, usages);
        }
        ExprKind::Match { expr: inner, arms } => {
            collect_usages_from_expr(inner, usages);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_usages_from_expr(guard, usages);
                }
                collect_usages_from_expr(&arm.body, usages);
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(s) = start {
                collect_usages_from_expr(s, usages);
            }
            if let Some(e) = end {
                collect_usages_from_expr(e, usages);
            }
        }
        ExprKind::Block(stmts) => {
            for s in stmts {
                collect_usages_from_stmt(s, usages);
            }
        }
        ExprKind::Spread(inner) | ExprKind::Paren(inner) => {
            collect_usages_from_expr(inner, usages);
        }
        _ => {}
    }
}

// Public API function for easy linting
pub fn lint(source: &str) -> Vec<LintDiagnostic> {
    lint_with_config(source, &LintConfig::default())
}

// Public API function for linting with config
pub fn lint_with_config(source: &str, config: &LintConfig) -> Vec<LintDiagnostic> {
    let linter = Linter::with_config(config.clone());
    linter.lint(source)
}

// Function to format diagnostics for display
pub fn format_diagnostics(diagnostics: &[LintDiagnostic]) -> String {
    let mut result = String::new();

    for diag in diagnostics {
        let severity = match diag.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
            Severity::Info => "INFO",
        };

        result.push_str(&format!(
            "[{}] {}: {} ({}:{})\n",
            severity, diag.rule_name, diag.message, diag.line, diag.column
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
