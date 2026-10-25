//! Document highlights, rename checks, selection ranges, workspace symbol
//! search and range formatting.

use crate::semantic::usage_finder::find_usages;
use crate::{Symbol, SymbolKind, extract_symbols, span_to_range, uri_to_path};
use pleno_ast::visitor::{self, Visitor};
use pleno_ast::{Expr, FieldDecl, FunctionDecl, Param, Pattern, Program, Span, Stmt};
use pleno_formatter::{FormatOptions, format_code};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Names the runtime provides; they have no declaration to rename.
const BUILTINS: &[&str] = &[
    "print", "println", "id", "typeof", "map", "Int", "Float", "String", "Array", "Map", "JSON",
    "Math", "OS", "Promise", "Channel", "Mutex", "spawn", "await",
];

/// Workspace symbol search stops after this many results or files.
const MAX_WORKSPACE_SYMBOLS: usize = 1000;
const MAX_WORKSPACE_FILES: usize = 5000;

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The byte span of the identifier at `offset`, including when `offset` is
/// just after it (where the cursor is after typing a name).
pub fn word_span(text: &str, offset: usize) -> Option<Span> {
    let offset = offset.min(text.len());
    let start = text[..offset]
        .char_indices()
        .rev()
        .take_while(|&(_, c)| is_ident_char(c))
        .last()
        .map_or(offset, |(i, _)| i);
    let end = text[offset..]
        .char_indices()
        .find(|&(_, c)| !is_ident_char(c))
        .map_or(text.len(), |(i, _)| offset + i);
    (start < end).then_some(Span::new(start, end))
}

/// Every occurrence of the name at `offset` that resolves to the same
/// declaration: the declaration is a write, the uses are reads.
pub fn document_highlights(program: &Program, text: &str, offset: usize) -> Vec<Value> {
    let Some(word) = word_span(text, offset) else {
        return Vec::new();
    };
    let usages = find_usages(program, word.start);
    usages
        .iter()
        .enumerate()
        .map(|(i, span)| {
            let (start, end) = span_to_range(text, span.start, span.end);
            // DocumentHighlightKind: 2 = Read, 3 = Write.
            json!({ "range": { "start": start, "end": end }, "kind": if i == 0 { 3 } else { 2 } })
        })
        .collect()
}

/// The range and current name of the symbol at `offset` if it can be renamed,
/// or why it cannot.
pub fn prepare_rename(program: &Program, text: &str, offset: usize) -> Result<Value, String> {
    let word = word_span(text, offset).ok_or("Only names can be renamed.")?;
    let name = &text[word.start..word.end];
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err("Only names can be renamed.".to_string());
    }
    if is_keyword(name) {
        return Err(format!("'{name}' is a keyword and cannot be renamed."));
    }
    if find_usages(program, word.start).is_empty() {
        return Err(if BUILTINS.contains(&name) {
            format!("'{name}' is built in and cannot be renamed.")
        } else {
            format!("No declaration of '{name}' was found in this file.")
        });
    }
    let (start, end) = span_to_range(text, word.start, word.end);
    Ok(json!({ "range": { "start": start, "end": end }, "placeholder": name }))
}

/// Whether the lexer reads `word` as a keyword or a literal like `true`.
fn is_keyword(word: &str) -> bool {
    use pleno_lexer::TokenKind;
    match pleno_lexer::lex(word).as_deref() {
        Ok([token, ..]) => {
            token.kind.keyword_text().is_some()
                || matches!(token.kind, TokenKind::Bool(_) | TokenKind::Null)
        }
        _ => false,
    }
}

/// Collects the spans of every node that contains an offset.
struct EnclosingSpans {
    offset: usize,
    spans: Vec<Span>,
}

impl EnclosingSpans {
    /// Records `span` if it contains the offset; returns whether it does. The
    /// end is inclusive so a cursor just after a name still selects it.
    fn record(&mut self, span: Span) -> bool {
        let inside = span.start <= self.offset && self.offset <= span.end;
        if inside && span.start < span.end {
            self.spans.push(span);
        }
        inside
    }
}

impl<'a> Visitor<'a> for EnclosingSpans {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if self.record(stmt.span) {
            visitor::walk_stmt(self, stmt);
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if self.record(expr.span) {
            visitor::walk_expr(self, expr);
        }
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        if self.record(pattern.span) {
            visitor::walk_pattern(self, pattern);
        }
    }

    fn visit_param(&mut self, param: &'a Param) {
        self.record(param.name_span);
        if self.record(param.span) {
            visitor::walk_param(self, param);
        }
    }

    fn visit_field_decl(&mut self, field: &'a FieldDecl) {
        if self.record(field.span) {
            self.record(field.name_span);
            visitor::walk_field_decl(self, field);
        }
    }

    fn visit_function_decl(&mut self, decl: &'a FunctionDecl) {
        if self.record(decl.span) {
            self.record(decl.name_span);
            visitor::walk_function_decl(self, decl);
        }
    }
}

/// The nested ranges around `offset`, innermost first with each `parent`
/// enclosing it: the name, then each enclosing expression and statement.
pub fn selection_range(program: &Program, text: &str, offset: usize) -> Value {
    let mut collector = EnclosingSpans {
        offset,
        spans: Vec::new(),
    };
    collector.visit_program(program);
    let mut spans = collector.spans;
    if let Some(word) = word_span(text, offset) {
        spans.push(word);
    }
    // The outermost step selects the whole document.
    spans.push(Span::new(0, text.len()));
    spans.sort_by_key(|s| std::cmp::Reverse(s.end - s.start));

    // Keep a chain where every range is inside the previous one.
    let mut chain: Vec<Span> = Vec::new();
    for span in spans {
        if chain.last().is_none_or(|outer| {
            outer.start <= span.start && span.end <= outer.end && *outer != span
        }) {
            chain.push(span);
        }
    }

    let mut result = Value::Null;
    for span in chain {
        let (start, end) = span_to_range(text, span.start, span.end);
        let mut range = json!({ "range": { "start": start, "end": end } });
        if !result.is_null() {
            range["parent"] = result;
        }
        result = range;
    }
    if result.is_null() {
        let (start, end) = span_to_range(text, offset, offset);
        result = json!({ "range": { "start": start, "end": end } });
    }
    result
}

/// Whether `query` matches `name`: its characters appear in order, ignoring
/// case, as editors' fuzzy symbol search does.
fn fuzzy_match(name: &str, query: &str) -> bool {
    let mut chars = name.chars().flat_map(char::to_lowercase);
    query
        .chars()
        .flat_map(char::to_lowercase)
        .all(|q| chars.any(|c| c == q))
}

fn lsp_symbol_kind(kind: SymbolKind) -> u32 {
    match kind {
        SymbolKind::Function => 12,
        SymbolKind::Method => 6,
        SymbolKind::Class => 5,
        SymbolKind::Interface => 11,
        SymbolKind::Variable => 13,
        SymbolKind::Field => 8,
    }
}

fn push_symbols(
    out: &mut Vec<Value>,
    symbols: &[Symbol],
    text: &str,
    uri: &str,
    query: &str,
    container: Option<&str>,
) {
    for sym in symbols {
        if out.len() >= MAX_WORKSPACE_SYMBOLS {
            return;
        }
        if fuzzy_match(&sym.name, query) {
            let (start, end) = span_to_range(text, sym.span.start, sym.span.end);
            let mut info = json!({
                "name": sym.name,
                "kind": lsp_symbol_kind(sym.kind),
                "location": { "uri": uri, "range": { "start": start, "end": end } }
            });
            if let Some(container) = container {
                info["containerName"] = json!(container);
            }
            out.push(info);
        }
        push_symbols(out, &sym.children, text, uri, query, Some(&sym.name));
    }
}

/// A path compared the way the file system does (case-insensitively on Windows).
fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// Every `.pln` file under `root`, skipping hidden, build and dependency folders.
fn pleno_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    pending.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "pln") {
                files.push(path);
                if files.len() >= MAX_WORKSPACE_FILES {
                    return files;
                }
            }
        }
    }
    files.sort();
    files
}

/// Top-level symbols (and class members) matching `query`, from the open
/// documents and from the `.pln` files under the workspace root.
pub fn workspace_symbols(state: &crate::ServerState, query: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut uris: Vec<&String> = state.index.keys().collect();
    uris.sort();
    for uri in uris {
        if let Some(path) = uri_to_path(uri) {
            seen.insert(path_key(&path));
        }
        if let Some(text) = state.documents.get(uri) {
            push_symbols(&mut out, &state.index[uri], text, uri, query, None);
        }
    }

    let root = state.root_uri.as_deref().and_then(uri_to_path);
    for path in root.as_deref().map(pleno_files).unwrap_or_default() {
        if out.len() >= MAX_WORKSPACE_SYMBOLS {
            break;
        }
        if !seen.insert(path_key(&path)) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(tokens) = pleno_lexer::lex(&text) else {
            continue;
        };
        let Ok(program) = pleno_parser::Parser::new(tokens, &text).parse_program() else {
            continue;
        };
        let Ok(uri) = url::Url::from_file_path(&path) else {
            continue;
        };
        let symbols = extract_symbols(&program, &path);
        push_symbols(&mut out, &symbols, &text, uri.as_str(), query, None);
    }
    out
}

/// Formats the top-level statements that `start..end` touches. The formatter
/// works on whole programs, so a selection grows to the declarations around
/// it; each of those is valid on its own and starts at column 0.
pub fn format_range(program: &Program, text: &str, start: usize, end: usize) -> Vec<Value> {
    let touched: Vec<&Stmt> = program
        .statements
        .iter()
        .filter(|s| s.span.start <= end && start <= s.span.end)
        .collect();
    let (Some(first), Some(last)) = (touched.first(), touched.last()) else {
        return Vec::new();
    };
    let from = first.span.start;
    let to = crate::lint::statement_end(text, last.span.end);
    let original = &text[from..to];
    let Ok(formatted) = format_code(original, &FormatOptions::default()) else {
        return Vec::new();
    };
    let formatted = formatted.trim_end_matches('\n');
    if formatted == original {
        return Vec::new();
    }
    let (start, end) = span_to_range(text, from, to);
    vec![json!({ "range": { "start": start, "end": end }, "newText": formatted })]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Program {
        let tokens = pleno_lexer::lex(source).unwrap();
        pleno_parser::Parser::new(tokens, source)
            .parse_program()
            .unwrap()
    }

    #[test]
    fn words_include_a_cursor_just_after_them() {
        let text = "let count = 1;";
        assert_eq!(word_span(text, 4), Some(Span::new(4, 9)));
        assert_eq!(word_span(text, 9), Some(Span::new(4, 9)));
        assert_eq!(word_span(text, 10), None);
    }

    #[test]
    fn keywords_and_builtins_cannot_be_renamed() {
        let text = "let count = 1;\nprint(count);\n";
        let program = parse(text);
        assert!(
            prepare_rename(&program, text, 1)
                .unwrap_err()
                .contains("keyword")
        );
        assert!(
            prepare_rename(&program, text, 16)
                .unwrap_err()
                .contains("built in")
        );
        let ok = prepare_rename(&program, text, 6).unwrap();
        assert_eq!(ok["placeholder"], "count");
    }

    #[test]
    fn selection_ranges_nest() {
        let text = "fn f(a) {\n    return a + 1;\n}\n";
        let program = parse(text);
        let offset = text.find("a + 1").unwrap();
        let mut range = &selection_range(&program, text, offset);
        let mut lengths = Vec::new();
        while !range.is_null() {
            let r = &range["range"];
            lengths.push((
                r["start"]["line"].as_u64().unwrap(),
                r["start"]["character"].as_u64().unwrap(),
                r["end"]["line"].as_u64().unwrap(),
                r["end"]["character"].as_u64().unwrap(),
            ));
            range = &range["parent"];
        }
        // `a`, `a + 1`, the return statement, ..., the whole document.
        assert_eq!(lengths[0], (1, 11, 1, 12));
        assert_eq!(lengths[1], (1, 11, 1, 16));
        assert_eq!(*lengths.last().unwrap(), (0, 0, 3, 0));
    }

    #[test]
    fn range_formatting_formats_the_touched_statements() {
        let text = "let a   =  1;\nlet b=2;\n";
        let program = parse(text);
        let edits = format_range(&program, text, 0, 3);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0]["newText"], "let a = 1;");
        assert_eq!(edits[0]["range"]["end"]["character"], 13);
    }

    #[test]
    fn fuzzy_matching_ignores_case() {
        assert!(fuzzy_match("parseConfig", "pcfg"));
        assert!(fuzzy_match("Point", ""));
        assert!(!fuzzy_match("Point", "pz"));
    }
}
