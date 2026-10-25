//! Linter settings sent by the client, lint results as LSP diagnostics, and
//! the quick fixes offered for them.

use crate::semantic::ast_utils::{Node, find_node_at};
use pleno_ast::{Expr, ExprKind, ImportKind, Program, Stmt, StmtKind};
use pleno_linter::{LintConfig, LintDiagnostic, Linter};
use serde_json::{Value, json};
use std::collections::HashSet;

/// The `pleno.lint.*` settings: `{ enabled, maxLineLength, rules: { name: bool } }`.
#[derive(Debug, Clone, PartialEq)]
pub struct LintSettings {
    pub enabled: bool,
    /// `None` turns the `max-line-length` rule off.
    pub max_line_length: Option<usize>,
    pub disabled_rules: HashSet<String>,
}

impl Default for LintSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_line_length: Some(100),
            disabled_rules: HashSet::new(),
        }
    }
}

impl LintSettings {
    /// Reads the `lint` settings object. Missing fields keep their defaults and
    /// a `maxLineLength` of 0 turns the rule off.
    pub fn from_json(value: &Value) -> Self {
        let mut settings = Self::default();
        if let Some(enabled) = value.get("enabled").and_then(|v| v.as_bool()) {
            settings.enabled = enabled;
        }
        if let Some(length) = value.get("maxLineLength").and_then(|v| v.as_u64()) {
            settings.max_line_length = (length > 0).then_some(length as usize);
        }
        if let Some(rules) = value.get("rules").and_then(|v| v.as_object()) {
            for (name, on) in rules {
                if on.as_bool() == Some(false) {
                    settings.disabled_rules.insert(name.clone());
                }
            }
        }
        settings
    }

    /// The linter configuration: rules the linter can switch off itself are
    /// turned off there, the rest are filtered out by `allows`.
    pub fn config(&self) -> LintConfig {
        let mut config = LintConfig::default();
        let rules = &mut config.rules;
        let on = |name: &str| !self.disabled_rules.contains(name);
        rules.max_line_length = self.max_line_length.filter(|_| on("max-line-length"));
        rules.warn_unused_vars &= on("no-unused-vars");
        rules.warn_trailing_spaces &= on("no-trailing-spaces");
        rules.warn_semi_spacing &= on("semi-spacing");
        rules.no_undef &= on("no-undef");
        rules.warn_shadowed_vars &= on("no-shadow");
        config
    }

    pub fn allows(&self, rule: &str) -> bool {
        // The server reports syntax errors itself; the linter's copies would
        // show every one of them twice.
        rule != "syntax-error" && rule != "parse-error" && !self.disabled_rules.contains(rule)
    }
}

/// Lints `text` and converts the results to LSP diagnostics.
pub fn lint_diagnostics(text: &str, uri: &str, settings: &LintSettings) -> Vec<Value> {
    if !settings.enabled {
        return Vec::new();
    }
    let linter = Linter::with_config(settings.config());
    let mut diagnostics = Vec::new();
    for ld in linter.lint(text) {
        if !settings.allows(&ld.rule_name) {
            continue;
        }
        let mut related = Vec::new();
        for ri in &ld.related_information {
            let line = ri.line.saturating_sub(1);
            let character = utf16_column(text, line, ri.column.saturating_sub(1));
            related.push(json!({
                "location": {
                    "uri": ri.file.clone().unwrap_or_else(|| uri.to_string()),
                    "range": {
                        "start": { "line": line, "character": character },
                        "end": { "line": line, "character": character + 1 }
                    }
                },
                "message": ri.message
            }));
        }

        let (line, start, end) = lint_range(text, &ld);
        let mut diag = json!({
            "range": {
                "start": { "line": line, "character": start },
                "end": { "line": line, "character": end }
            },
            "severity": match ld.severity {
                pleno_linter::Severity::Error => 1,
                pleno_linter::Severity::Warning => 2,
                pleno_linter::Severity::Info => 3,
            },
            "source": "pleno-lint",
            "code": ld.rule_name,
            "message": ld.message
        });
        let object = diag.as_object_mut().unwrap();
        if ld.rule_name == "no-unused-vars" {
            // Unnecessary: the editor fades the name out.
            object.insert("tags".to_string(), json!([1]));
        }
        if !related.is_empty() {
            object.insert("relatedInformation".to_string(), json!(related));
        }
        diagnostics.push(diag);
    }
    diagnostics
}

/// The range a lint diagnostic covers, as (line, start, end) in LSP units.
/// When the linter reports only a start position, this widens it to the whole
/// name or operator the message quotes, or to the offending part of the line.
fn lint_range(text: &str, ld: &LintDiagnostic) -> (usize, usize, usize) {
    let line = ld.line.saturating_sub(1);
    let content = line_text(text, line);
    let chars: Vec<char> = content.chars().collect();
    let col = ld.column.saturating_sub(1).min(chars.len());
    // The exclusive end the linter reports (0-based).
    let reported_end = col + 1;

    let (start, end) = match ld.rule_name.as_str() {
        _ if reported_end > col + 1 => (col, reported_end),
        "max-line-length" => (col, chars.len()),
        "no-trailing-spaces" => (content.trim_end().chars().count(), chars.len()),
        _ => match quoted(&ld.message) {
            Some(needle) => find_near(&chars, &needle, col).unwrap_or((col, col + 1)),
            None => word_around(&chars, col).unwrap_or((col, col + 1)),
        },
    };
    let end = end.min(chars.len()).max(start);
    let to_utf16 = |n: usize| {
        chars[..n.min(chars.len())]
            .iter()
            .map(|c| c.len_utf16())
            .sum()
    };
    // A position past the end of the line (an empty line, say) still gets a
    // one-character range, which editors clamp.
    let start16: usize = to_utf16(start);
    let end16: usize = if end > start {
        to_utf16(end)
    } else {
        start16 + 1
    };
    (line, start16, end16)
}

/// The first `'...'` quoted text in a message, which names the variable or
/// operator the diagnostic is about.
fn quoted(message: &str) -> Option<String> {
    let start = message.find('\'')? + 1;
    let len = message[start..].find('\'')?;
    (len > 0).then(|| message[start..start + len].to_string())
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Finds `needle` in `chars` at `col`, else the nearest match after it, else
/// the last one before it. Identifiers only match as whole words.
fn find_near(chars: &[char], needle: &str, col: usize) -> Option<(usize, usize)> {
    let needle: Vec<char> = needle.chars().collect();
    let word = needle.iter().all(|&c| is_ident_char(c));
    let matches = |at: usize| {
        at + needle.len() <= chars.len()
            && chars[at..at + needle.len()] == needle[..]
            && (!word
                || ((at == 0 || !is_ident_char(chars[at - 1]))
                    && chars
                        .get(at + needle.len())
                        .is_none_or(|&c| !is_ident_char(c))))
    };
    let found = (col..chars.len())
        .find(|&at| matches(at))
        .or_else(|| (0..col).rev().find(|&at| matches(at)))?;
    Some((found, found + needle.len()))
}

/// The identifier that `col` is in or just after.
fn word_around(chars: &[char], col: usize) -> Option<(usize, usize)> {
    let mut start = col.min(chars.len());
    while start > 0 && is_ident_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = col.min(chars.len());
    while end < chars.len() && is_ident_char(chars[end]) {
        end += 1;
    }
    (start < end).then_some((start, end))
}

/// Line `line` (0-based) without its line ending.
fn line_text(text: &str, line: usize) -> &str {
    text.split('\n')
        .nth(line)
        .map(|l| l.trim_end_matches('\r'))
        .unwrap_or("")
}

/// The UTF-16 column of the character at index `col` of line `line`.
fn utf16_column(text: &str, line: usize, col: usize) -> usize {
    line_text(text, line)
        .chars()
        .take(col)
        .map(|c| c.len_utf16())
        .sum()
}

/// Quick fixes for one diagnostic from the client. `text` and `program` are
/// the current document and its parse.
pub fn quick_fixes(diag: &Value, text: &str, program: Option<&Program>, uri: &str) -> Vec<Value> {
    let code = diag.get("code").and_then(|c| c.as_str()).unwrap_or("");
    let message = diag.get("message").and_then(|m| m.as_str()).unwrap_or("");
    let Some((start, end)) = diag_offsets(diag, text) else {
        return Vec::new();
    };
    let covered = &text[start..end];
    let fix = |title: String, edits: Vec<Value>, preferred: bool| {
        json!({
            "title": title,
            "kind": "quickfix",
            "diagnostics": [diag],
            "isPreferred": preferred,
            "edit": { "changes": { uri: edits } }
        })
    };

    let mut actions = Vec::new();
    if code == "no-unused-vars" || message.contains("is declared but never used") {
        let Some(name) = quoted(message).filter(|name| name == covered) else {
            return actions;
        };
        match program.and_then(|p| find_node_at(p, start)) {
            Some(Node::Stmt(stmt)) if matches!(stmt.kind, StmtKind::Import { .. }) => {
                if let Some(edit) = remove_import_name(stmt, text, &name) {
                    actions.push(fix(
                        format!("Remove unused import '{name}'"),
                        vec![edit],
                        true,
                    ));
                }
            }
            Some(Node::Stmt(stmt)) => {
                if let StmtKind::Let {
                    name_span, value, ..
                } = &stmt.kind
                    && name_span.start == start
                    && value.as_ref().is_none_or(is_pure)
                {
                    let edit =
                        delete_edit(text, stmt.span.start, statement_end(text, stmt.span.end));
                    actions.push(fix(
                        format!("Remove unused variable '{name}'"),
                        vec![edit],
                        false,
                    ));
                }
                actions.push(prefix_fix(&fix, text, start, &name));
            }
            _ => actions.push(prefix_fix(&fix, text, start, &name)),
        }
    } else if code == "operator-spacing"
        && let Some(op) = quoted(message).filter(|op| op == covered)
    {
        let mut edits = Vec::new();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if before.is_some_and(|c| !c.is_whitespace()) {
            edits.push(insert_edit(text, start, " "));
        }
        if after.is_some_and(|c| !c.is_whitespace()) {
            edits.push(insert_edit(text, end, " "));
        }
        if !edits.is_empty() {
            actions.push(fix(format!("Add spaces around '{op}'"), edits, true));
        }
    }
    actions
}

/// The fix that renames an unused name to `_name`, which the linter treats as
/// intentionally unused.
fn prefix_fix(
    fix: &dyn Fn(String, Vec<Value>, bool) -> Value,
    text: &str,
    start: usize,
    name: &str,
) -> Value {
    fix(
        format!("Prefix '{name}' with '_' to mark it as unused"),
        vec![insert_edit(text, start, "_")],
        true,
    )
}

/// The byte offsets of a diagnostic's range in `text`.
fn diag_offsets(diag: &Value, text: &str) -> Option<(usize, usize)> {
    let offset = |key: &str| {
        let pos = diag.get("range")?.get(key)?;
        crate::position_to_offset(
            text,
            pos.get("line")?.as_u64()? as u32,
            pos.get("character")?.as_u64()? as u32,
        )
    };
    let (start, end) = (offset("start")?, offset("end")?);
    (start <= end).then_some((start, end))
}

/// Whether evaluating `expr` has no effect, so a `let` of it can be deleted.
fn is_pure(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Ident(_) | ExprKind::This => true,
        ExprKind::Paren(e) | ExprKind::Unary { expr: e, .. } => is_pure(e),
        ExprKind::ArrayLiteral(items) | ExprKind::TupleLiteral(items) => items.iter().all(is_pure),
        _ => false,
    }
}

/// The edit that removes the unused `name` from an import: the whole statement
/// when it is the only name imported, otherwise just that name.
fn remove_import_name(stmt: &Stmt, text: &str, name: &str) -> Option<Value> {
    let StmtKind::Import { kind, .. } = &stmt.kind else {
        return None;
    };
    let end = statement_end(text, stmt.span.end);
    if let ImportKind::Names(items) = kind
        && items.len() > 1
    {
        let source = &text[stmt.span.start..end];
        let open = source.find('{')?;
        let close = source.rfind('}')?;
        let kept: Vec<&str> = source[open + 1..close]
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .filter(|item| item.split_whitespace().last() != Some(name))
            .collect();
        if kept.len() + 1 != items.len() {
            return None;
        }
        let rewritten = format!(
            "{}{{ {} }}{}",
            &source[..open],
            kept.join(", "),
            &source[close + 1..]
        );
        return Some(replace_edit(text, stmt.span.start, end, &rewritten));
    }
    Some(delete_edit(text, stmt.span.start, end))
}

/// Statement spans stop before the `;`: extends `end` past one if it follows.
pub(crate) fn statement_end(text: &str, end: usize) -> usize {
    let rest = &text[end..];
    let trimmed = rest.trim_start_matches([' ', '\t']);
    if trimmed.starts_with(';') {
        end + (rest.len() - trimmed.len()) + 1
    } else {
        end
    }
}

/// Deletes `start..end`, and the whole line when nothing else is on it.
fn delete_edit(text: &str, start: usize, end: usize) -> Value {
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
    if text[line_start..start].trim().is_empty() && text[end..line_end].trim().is_empty() {
        replace_edit(text, line_start, line_end, "")
    } else {
        replace_edit(text, start, end, "")
    }
}

fn insert_edit(text: &str, at: usize, new_text: &str) -> Value {
    replace_edit(text, at, at, new_text)
}

fn replace_edit(text: &str, start: usize, end: usize, new_text: &str) -> Value {
    let (start, end) = crate::span_to_range(text, start, end);
    json!({ "range": { "start": start, "end": end }, "newText": new_text })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag(rule: &str, message: &str, line: usize, column: usize) -> LintDiagnostic {
        LintDiagnostic {
            rule_name: rule.to_string(),
            message: message.to_string(),
            severity: pleno_linter::Severity::Warning,
            line,
            column,
            related_information: vec![],
        }
    }

    #[test]
    fn ranges_cover_the_quoted_name() {
        let text = "let total = 1;\nimport { a, bb } from \"m\";\n";
        let unused = diag(
            "no-unused-vars",
            "Variable 'total' is declared but never used",
            1,
            5,
        );
        assert_eq!(lint_range(text, &unused), (0, 4, 9));
        // Imports are reported at the `import` keyword: the name is found on the line.
        let import = diag(
            "no-unused-vars",
            "Variable 'bb' is declared but never used",
            2,
            1,
        );
        assert_eq!(lint_range(text, &import), (1, 12, 14));
        let long = diag("max-line-length", "Line too long", 1, 4);
        assert_eq!(lint_range(text, &long), (0, 3, 14));
    }

    #[test]
    fn settings_map_to_the_linter_config() {
        let settings = LintSettings::from_json(&json!({
            "maxLineLength": 0,
            "rules": { "no-shadow": false, "operator-spacing": false, "no-undef": true }
        }));
        let config = settings.config();
        assert_eq!(config.rules.max_line_length, None);
        assert!(!config.rules.warn_shadowed_vars);
        assert!(config.rules.no_undef);
        assert!(!settings.allows("operator-spacing"));
        assert!(settings.allows("no-undef"));
        assert_eq!(LintSettings::from_json(&json!({})), LintSettings::default());
    }

    #[test]
    fn import_names_are_removed_one_at_a_time() {
        let text = "import { a, b as c } from \"m\";\nprint(a);\n";
        let tokens = pleno_lexer::lex(text).unwrap();
        let program = pleno_parser::Parser::new(tokens, text)
            .parse_program()
            .unwrap();
        let edit = remove_import_name(&program.statements[0], text, "c").unwrap();
        assert_eq!(edit["newText"], "import { a } from \"m\";");
        let edit = remove_import_name(&program.statements[0], text, "a").unwrap();
        assert_eq!(edit["newText"], "import { b as c } from \"m\";");
    }
}
