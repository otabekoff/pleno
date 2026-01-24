use pleno_ast::*;
use pleno_lexer::lex;
use pleno_parser::Parser;

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
            diagnostics.extend(self.check_unused_variables(&ast));
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

    fn check_unused_variables(&self, program: &Program) -> Vec<LintDiagnostic> {
        let mut diagnostics = Vec::new();

        fn find_let_declarations(stmt: &Stmt, diagnostics: &mut Vec<LintDiagnostic>) {
            match &stmt.kind {
                StmtKind::Let { name, .. } => {
                    diagnostics.push(LintDiagnostic {
                        rule_name: "no-unused-vars".to_string(),
                        message: format!("Variable '{}' is declared but possibly unused", name),
                        severity: Severity::Warning,
                        line: 0,
                        column: 0,
                    });
                }
                StmtKind::Block(stmts) => {
                    for s in stmts {
                        find_let_declarations(s, diagnostics);
                    }
                }
                StmtKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    find_let_declarations(then_branch, diagnostics);
                    if let Some(eb) = else_branch {
                        find_let_declarations(eb, diagnostics);
                    }
                }
                StmtKind::While { body, .. } => find_let_declarations(body, diagnostics),
                StmtKind::For { body, .. } => find_let_declarations(body, diagnostics),
                StmtKind::Fn { body, .. } => {
                    find_let_declarations(body, diagnostics);
                }
                _ => {}
            }
        }

        for stmt in &program.statements {
            find_let_declarations(stmt, &mut diagnostics);
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

                    // Simple check: is there a space before and after?
                    // We avoid false positives for composite operators like ==, >=, etc.
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
                            // One warning per line per operator is enough for now to avoid spam
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

    // Method to add custom rules
    pub fn add_rule(&mut self, rule: LintRule) {
        self.rules.push(rule);
    }

    // Method to run specific rules
    pub fn run_rules(&self, source: &str, rule_names: &[&str]) -> Vec<LintDiagnostic> {
        let mut all_diagnostics = self.lint(source);

        all_diagnostics.retain(|diag| rule_names.contains(&diag.rule_name.as_str()));

        all_diagnostics
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
