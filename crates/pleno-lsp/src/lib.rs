use pleno_ast::{Program, Span, StmtKind};
// use pleno_lexer::Token; // Unused
use pleno_formatter::{FormatOptions, format_code};
use pleno_linter::Linter;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// --- Data Structures ---

#[derive(Debug, Clone)]
struct Symbol {
    name: String,
    kind: SymbolKind,
    span: Span,
    detail: Option<String>,
    #[allow(dead_code)]
    file_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SymbolKind {
    Function,
    Class,
    Interface,
    Variable,
    Field,
    Method,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticToken {
    line: u32,
    char: u32,
    length: u32,
    token_type: u32,
    token_modifiers: u32,
}

struct ServerState {
    // Map URI -> File Content
    documents: HashMap<String, String>,
    // Map URI -> Parsed Symbols
    index: HashMap<String, Vec<Symbol>>,
    #[allow(dead_code)]
    root_uri: Option<String>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            index: HashMap::new(),
            root_uri: None,
        }
    }

    fn update_document(&mut self, uri: String, text: String) -> Vec<Value> {
        self.documents.insert(uri.clone(), text.clone());

        let mut diagnostics = Vec::new();
        // Lexing
        let tokens = match pleno_lexer::lex(&text) {
            Ok(t) => t,
            Err(e) => {
                let (start, end) = span_to_range(&text, e.span.start, e.span.end);
                diagnostics.push(json!({
                    "range": { "start": start, "end": end },
                    "severity": 1,
                    "message": e.message
                }));
                return diagnostics;
            }
        };

        // Linting
        let linter = Linter::default();
        let lint_diags = linter.lint(&text);
        for ld in lint_diags {
            diagnostics.push(json!({
                "range": {
                    "start": { "line": ld.line.saturating_sub(1), "character": ld.column.saturating_sub(1) },
                    "end": { "line": ld.line.saturating_sub(1), "character": ld.column }
                },
                "severity": match ld.severity {
                    pleno_linter::Severity::Error => 1,
                    pleno_linter::Severity::Warning => 2,
                    pleno_linter::Severity::Info => 3,
                },
                "message": ld.message
            }));
        }

        // Parsing
        let mut parser = pleno_parser::Parser::new(tokens, &text);
        match parser.parse_program() {
            Ok(program) => {
                // Index symbols
                if let Some(path) = uri_to_path(&uri) {
                    let symbols = extract_symbols(&program, &path);

                    // Check for re-definitions in this file
                    let mut seen_names: std::collections::HashMap<String, Symbol> =
                        std::collections::HashMap::new();
                    for sym in &symbols {
                        if let Some(original) = seen_names.get(&sym.name) {
                            let (start, end) = span_to_range(&text, sym.span.start, sym.span.end);
                            let (orig_start, orig_end) =
                                span_to_range(&text, original.span.start, original.span.end);
                            diagnostics.push(json!({
                                "range": { "start": start, "end": end },
                                "severity": 1,
                                "message": format!("Symbol '{}' is already defined", sym.name),
                                "relatedInformation": [
                                    {
                                        "location": {
                                            "uri": uri,
                                            "range": { "start": orig_start, "end": orig_end }
                                        },
                                        "message": "Original declaration is here"
                                    }
                                ]
                            }));
                        } else {
                            seen_names.insert(sym.name.clone(), sym.clone());
                        }
                    }

                    self.index.insert(uri, symbols);
                }
            }
            Err(e) => {
                let (start, end) = span_to_range(&text, e.span.start, e.span.end);
                diagnostics.push(json!({
                    "range": { "start": start, "end": end },
                    "severity": 1,
                    "message": e.message
                }));
            }
        }

        diagnostics
    }
}

// --- Main Loop ---

pub fn start() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut stdout = io::stdout();

    let state = Arc::new(Mutex::new(ServerState::new()));

    while let Some(message) = read_message(&mut reader)? {
        let value: Value = match serde_json::from_str(&message) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = match value.get("method").and_then(|v| v.as_str()) {
            Some(m) => m,
            None => {
                // Respond to requests with error if needed, but here just ignore invalid json-rpc
                continue;
            }
        };

        let state_clone = state.clone();

        match method {
            "initialize" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                // Extract rootUri
                let mut state = state_clone.lock().unwrap();
                if let Some(params) = value.get("params")
                    && let Some(root) = params.get("rootUri").and_then(|v| v.as_str())
                {
                    state.root_uri = Some(root.to_string());
                }

                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "textDocumentSync": 1, // Full sync
                            "definitionProvider": true,
                            "hoverProvider": true,
                            "documentFormattingProvider": true,
                            "inlayHintProvider": true,
                            "completionProvider": {
                                "resolveProvider": false,
                                "triggerCharacters": [".", ":"]
                            },
                            "documentSymbolProvider": true,
                            "foldingRangeProvider": true,
                            "semanticTokensProvider": {
                                "legend": {
                                    "tokenTypes": [
                                        "comment", "string", "keyword", "number", "variable", "parameter",
                                        "function", "method", "class", "interface", "property", "type"
                                    ],
                                    "tokenModifiers": ["declaration", "readonly", "static"]
                                },
                                "full": true
                            },
                            "codeActionProvider": true,
                            "codeLensProvider": {
                                "resolveProvider": false
                            },
                            "callHierarchyProvider": true
                        }
                    }
                });
                write_message(&mut stdout, &response)?;
            }
            "textDocument/didOpen" => {
                if let Some(params) = value.get("params") {
                    let uri = params
                        .get("textDocument")
                        .and_then(|td| td.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let text = params
                        .get("textDocument")
                        .and_then(|td| td.get("text").and_then(|v| v.as_str()))
                        .unwrap_or_default();

                    let mut state = state_clone.lock().unwrap();
                    let diags = state.update_document(uri.to_string(), text.to_string());

                    // Publish diagnostics
                    let notif = json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": {
                            "uri": uri,
                            "diagnostics": diags
                        }
                    });
                    write_message(&mut stdout, &notif)?;
                }
            }
            "textDocument/didChange" => {
                if let Some(params) = value.get("params") {
                    let uri = params
                        .get("textDocument")
                        .and_then(|td| td.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    // Full sync assumed
                    if let Some(changes) = params.get("contentChanges").and_then(|c| c.as_array())
                        && let Some(first) = changes.first()
                        && let Some(text) = first.get("text").and_then(|t| t.as_str())
                    {
                        let mut state = state_clone.lock().unwrap();
                        let diags = state.update_document(uri.to_string(), text.to_string());
                        drop(state); // unlock before writing

                        let notif = json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/publishDiagnostics",
                            "params": {
                                "uri": uri,
                                "diagnostics": diags
                            }
                        });
                        write_message(&mut stdout, &notif)?;
                    }
                }
            }
            "textDocument/definition" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let pos_json = params.get("position");

                    let mut result = Value::Null;

                    let state = state_clone.lock().unwrap();
                    // Ensure we have current file content
                    if let Some(text) = state.documents.get(uri_str).cloned() {
                        // We need to parse finding the token at cursor
                        if let Some(pos_val) = pos_json {
                            let line =
                                pos_val.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                            let char = pos_val
                                .get("character")
                                .and_then(|c| c.as_u64())
                                .unwrap_or(0) as u32;

                            if let Some(word) = get_word_at_position(&text, line, char) {
                                // 1. Search in global index
                                // Note: For real import resolution, we should resolve imports in this file.
                                // For now, simpler approach: search ALL indexed symbols with matching name.
                                // Improve later to respect scope/imports.

                                // Also lazily index other files if not in index?
                                // For now assume we only know about opened files or we'd need to walk dir.

                                // Let's try to match exact name
                                for (idx_uri, symbols) in &state.index {
                                    if let Some(sym) = symbols.iter().find(|s| s.name == word) {
                                        // Found definition!
                                        // Need to read that file to get line/col from span if not cached?
                                        // Wait, Symbol stores Span (byte offset).
                                        // We need content of that file to convert Span -> Line/Col.

                                        let file_content =
                                            if let Some(c) = state.documents.get(idx_uri) {
                                                c.clone()
                                            } else {
                                                // Try reading from disk
                                                if let Some(p) = uri_to_path(idx_uri) {
                                                    if let Ok(c) = fs::read_to_string(p) {
                                                        c
                                                    } else {
                                                        continue;
                                                    }
                                                } else {
                                                    continue;
                                                }
                                            };

                                        let (start, end) = span_to_range(
                                            &file_content,
                                            sym.span.start,
                                            sym.span.end,
                                        );

                                        result = json!({
                                            "uri": idx_uri,
                                            "range": {
                                                "start": start,
                                                "end": end
                                            }
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/hover" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let pos_json = params.get("position");

                    let mut result = Value::Null;
                    let state = state_clone.lock().unwrap();

                    if let Some(text) = state.documents.get(uri_str).cloned()
                        && let Some(pos_val) = pos_json
                    {
                        let line = pos_val.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                        let char = pos_val
                            .get("character")
                            .and_then(|c| c.as_u64())
                            .unwrap_or(0) as u32;

                        if let Some(word) = get_word_at_position(&text, line, char) {
                            // Look up symbol
                            let mut hover_text = String::new();
                            let mut found = false;

                            // Search index
                            for symbols in state.index.values() {
                                if let Some(sym) = symbols.iter().find(|s| s.name == word) {
                                    let kind_str = match sym.kind {
                                        SymbolKind::Function => "Function",
                                        SymbolKind::Class => "Class",
                                        SymbolKind::Interface => "Interface",
                                        SymbolKind::Variable => "Variable",
                                        SymbolKind::Field => "Field",
                                        SymbolKind::Method => "Method",
                                    };

                                    let detail = sym.detail.as_deref().unwrap_or("");
                                    hover_text = format!(
                                        "**{}** *({})*\n\n```pleno\n{}\n```",
                                        sym.name, kind_str, detail
                                    );
                                    found = true;
                                    break;
                                }
                            }

                            if !found {
                                // Default fallback
                                hover_text = format!("**{}** (local)", word);
                            }

                            result = json!({
                                "contents": {
                                    "kind": "markdown",
                                    "value": hover_text
                                }
                            });
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/formatting" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();

                    let state = state_clone.lock().unwrap();
                    let mut text_edit = Value::Null;

                    if let Some(text) = state.documents.get(uri_str)
                        && let Ok(formatted) = format_code(text, &FormatOptions::default())
                    {
                        let lines: Vec<&str> = text.lines().collect();
                        let last_line = lines.len().saturating_sub(1) as u32;
                        let last_char = lines.last().map(|l| l.len()).unwrap_or(0) as u32;

                        text_edit = json!([{
                            "range": {
                                "start": { "line": 0, "character": 0 },
                                "end": { "line": last_line, "character": last_char }
                            },
                            "newText": formatted
                        }]);
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": text_edit
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/inlayHint" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();

                    let state = state_clone.lock().unwrap();
                    let mut hints = Vec::new();

                    if let Some(symbols) = state.index.get(uri_str) {
                        for sym in symbols {
                            if sym.kind == SymbolKind::Variable
                                && let Some(detail) = &sym.detail
                                && let Some(ty_part) = detail.split(": ").last()
                            {
                                let source = state
                                    .documents
                                    .get(uri_str)
                                    .map(|s| s.as_str())
                                    .unwrap_or("");
                                let (line, col) = offset_to_position(source, sym.span.end);
                                hints.push(json!({
                                    "position": { "line": line, "character": col },
                                    "label": format!(": {}", ty_part),
                                    "kind": 1, // Type
                                    "paddingLeft": true
                                }));
                            }
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": hints
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/completion" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                let mut items = Vec::new();

                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let position = params.get("position");
                    let line = position
                        .and_then(|p| p.get("line"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let character = position
                        .and_then(|p| p.get("character"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;

                    let state = state_clone.lock().unwrap();
                    if let Some(text) = state.documents.get(uri_str) {
                        let lines: Vec<&str> = text.lines().collect();
                        if let Some(current_line) = lines.get(line as usize) {
                            let prefix = if (character as usize) <= current_line.len() {
                                &current_line[..character as usize]
                            } else {
                                ""
                            };

                            if prefix.trim_end().ends_with('.') {
                                // Property completion
                                let properties = [
                                    ("length", 10, "Returns the number of elements"),
                                    ("push", 2, "Adds an element to the end"),
                                    ("pop", 2, "Removes the last element"),
                                    ("keys", 2, "Returns an iterator over keys"),
                                    ("values", 2, "Returns an iterator over values"),
                                    ("entries", 2, "Returns an iterator over entries"),
                                    ("toLowerCase", 2, "Converts string to lower case"),
                                    ("toUpperCase", 2, "Converts string to upper case"),
                                    ("trim", 2, "Removes whitespace from both ends"),
                                    ("split", 2, "Splits string by a separator"),
                                    ("join", 2, "Joins array elements into a string"),
                                ];

                                for (name, kind, detail) in properties {
                                    items.push(json!({
                                        "label": name,
                                        "kind": kind,
                                        "detail": detail
                                    }));
                                }
                            } else {
                                // Normal completion (keywords + symbols)
                                let keywords = [
                                    "let",
                                    "fn",
                                    "if",
                                    "else",
                                    "while",
                                    "for",
                                    "in",
                                    "return",
                                    "class",
                                    "enum",
                                    "interface",
                                    "try",
                                    "catch",
                                    "finally",
                                    "throw",
                                    "match",
                                    "case",
                                    "default",
                                    "true",
                                    "false",
                                    "null",
                                    "import",
                                    "export",
                                    "async",
                                    "await",
                                    "spawn",
                                    "print",
                                    "println",
                                    "id",
                                    "typeof",
                                    "Int",
                                    "Float",
                                    "String",
                                    "Array",
                                    "Map",
                                    "JSON",
                                    "Math",
                                    "OS",
                                ];

                                for kw in keywords {
                                    items.push(json!({
                                        "label": kw,
                                        "kind": 14,
                                        "detail": "Keyword"
                                    }));
                                }

                                if let Some(symbols) = state.index.get(uri_str) {
                                    for sym in symbols {
                                        items.push(json!({
                                            "label": sym.name,
                                            "kind": match sym.kind {
                                                SymbolKind::Function | SymbolKind::Method => 3,
                                                SymbolKind::Class => 7,
                                                SymbolKind::Variable | SymbolKind::Field => 6,
                                                _ => 1,
                                            },
                                            "detail": sym.detail.as_deref().unwrap_or("")
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }

                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": items
                });
                write_message(&mut stdout, &resp)?;
            }
            "textDocument/documentSymbol" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();

                    let state = state_clone.lock().unwrap();
                    let mut symbols_out = Vec::new();

                    if let Some(symbols) = state.index.get(uri_str)
                        && let Some(text) = state.documents.get(uri_str)
                    {
                        for sym in symbols {
                            let (start, end) = span_to_range(text, sym.span.start, sym.span.end);
                            symbols_out.push(json!({
                                "name": sym.name,
                                "kind": match sym.kind {
                                    SymbolKind::Function | SymbolKind::Method => 12, // Function
                                    SymbolKind::Class => 5, // Class
                                    SymbolKind::Variable | SymbolKind::Field => 13, // Variable
                                    _ => 1,
                                },
                                "location": {
                                    "uri": uri_str,
                                    "range": { "start": start, "end": end }
                                },
                                "detail": sym.detail.as_deref().unwrap_or("")
                            }));
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": symbols_out
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/foldingRange" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();

                    let state = state_clone.lock().unwrap();
                    let mut ranges = Vec::new();

                    if let Some(text) = state.documents.get(uri_str) {
                        // Simple brace-based folding
                        let mut stack = Vec::new();
                        for (i, line) in text.lines().enumerate() {
                            for ch in line.chars() {
                                if ch == '{' {
                                    stack.push(i);
                                } else if ch == '}'
                                    && let Some(start_line) = stack.pop()
                                    && start_line < i
                                {
                                    ranges.push(json!({
                                        "startLine": start_line,
                                        "endLine": i,
                                        "kind": "region"
                                    }));
                                }
                            }
                        }

                        // Also look for multi-line comments
                        let mut comment_start = None;
                        for (i, line) in text.lines().enumerate() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("/*") && !trimmed.contains("*/") {
                                comment_start = Some(i);
                            } else if trimmed.contains("*/")
                                && let Some(start) = comment_start
                                && let Some(line_num) = Some(i)
                            {
                                ranges.push(json!({
                                    "startLine": start,
                                    "endLine": line_num,
                                    "kind": "comment"
                                }));
                                comment_start = None;
                            }
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": ranges
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/semanticTokens/full" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();

                    let state = state_clone.lock().unwrap();
                    let mut data = Vec::new();

                    if let Some(text) = state.documents.get(uri_str) {
                        // Re-parse or use cached AST? For now re-parse for simplicity
                        let tokens = pleno_lexer::lex(text).unwrap_or_default();
                        let mut parser = pleno_parser::Parser::new(tokens, text);
                        if let Ok(program) = parser.parse_program() {
                            let mut semantic_tokens = collect_semantic_tokens(&program, text);
                            // Sort tokens by line then char
                            semantic_tokens.sort_by(|a, b| {
                                if a.line != b.line {
                                    a.line.cmp(&b.line)
                                } else {
                                    a.char.cmp(&b.char)
                                }
                            });

                            let mut prev_line = 0;
                            let mut prev_char = 0;

                            for token in semantic_tokens {
                                let delta_line = token.line - prev_line;
                                let delta_start = if delta_line == 0 {
                                    token.char - prev_char
                                } else {
                                    token.char
                                };

                                data.push(delta_line);
                                data.push(delta_start);
                                data.push(token.length);
                                data.push(token.token_type);
                                data.push(token.token_modifiers);

                                prev_line = token.line;
                                prev_char = token.char;
                            }
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "data": data
                        }
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/codeAction" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let context = params.get("context");
                    let diagnostics = context
                        .and_then(|c| c.get("diagnostics"))
                        .and_then(|d| d.as_array());

                    let mut actions = Vec::new();

                    if let Some(diags) = diagnostics {
                        for diag in diags {
                            let message =
                                diag.get("message").and_then(|m| m.as_str()).unwrap_or("");
                            let range = diag.get("range").cloned().unwrap_or(Value::Null);

                            // Example 1: Missing semicolon
                            if message.contains("expected ';'") {
                                actions.push(json!({
                                    "title": "Add missing ';'",
                                    "kind": "quickfix",
                                    "diagnostics": [diag],
                                    "edit": {
                                        "changes": {
                                            uri_str: [{
                                                "range": {
                                                    "start": range.get("end").unwrap_or(&Value::Null),
                                                    "end": range.get("end").unwrap_or(&Value::Null)
                                                },
                                                "newText": ";"
                                            }]
                                        }
                                    }
                                }));
                            }

                            // Example 2: Unused variable (placeholder logic)
                            if message.contains("unused variable") {
                                actions.push(json!({
                                    "title": "Remove unused variable",
                                    "kind": "quickfix",
                                    "diagnostics": [diag],
                                    "edit": {
                                        "changes": {
                                            uri_str: [{
                                                "range": range,
                                                "newText": ""
                                            }]
                                        }
                                    }
                                }));
                            }
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": actions
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/codeLens" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let state = state_clone.lock().unwrap();
                    let text = state
                        .documents
                        .get(uri_str)
                        .map(|s| s.as_str())
                        .unwrap_or("");

                    let mut lenses = Vec::new();

                    // Re-parse or use cached AST?
                    let tokens = pleno_lexer::lex(text).unwrap_or_default();
                    let mut parser = pleno_parser::Parser::new(tokens, text);
                    if let Ok(program) = parser.parse_program() {
                        for stmt in &program.statements {
                            if let pleno_ast::StmtKind::Fn { .. } = &stmt.kind {
                                let (start, _) =
                                    span_to_range(text, stmt.span.start, stmt.span.end);
                                lenses.push(json!({
                                    "range": {
                                        "start": start,
                                        "end": start
                                    },
                                    "command": {
                                        "title": "$(play) Run Function",
                                        "command": "pleno.runFile",
                                        "arguments": [uri_str]
                                    }
                                }));
                            }
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": lenses
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/prepareCallHierarchy" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let pos_json = params.get("position");

                    let state = state_clone.lock().unwrap();
                    let mut items = Vec::new();

                    if let Some(text) = state.documents.get(uri_str).cloned()
                        && let Some(pos_val) = pos_json
                    {
                        let line = pos_val.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                        let char = pos_val
                            .get("character")
                            .and_then(|c| c.as_u64())
                            .unwrap_or(0) as u32;

                        if let Some(word) = get_word_at_position(&text, line, char)
                            && let Some(symbols) = state.index.get(uri_str)
                            && let Some(sym) = symbols.iter().find(|s| s.name == word)
                        {
                            let (start, end) = span_to_range(&text, sym.span.start, sym.span.end);
                            items.push(json!({
                                "name": sym.name,
                                "kind": match sym.kind {
                                    SymbolKind::Function | SymbolKind::Method => 12,
                                    SymbolKind::Class => 5,
                                    _ => 1,
                                },
                                "uri": uri_str,
                                "range": { "start": start, "end": end },
                                "selectionRange": { "start": start, "end": end }
                            }));
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": items
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "callHierarchy/incomingCalls" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let item = params.get("item");
                    let name = item
                        .and_then(|i| i.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");

                    let state = state_clone.lock().unwrap();
                    let mut incoming = Vec::new();

                    for (uri, text) in &state.documents {
                        let tokens = pleno_lexer::lex(text).unwrap_or_default();
                        let mut parser = pleno_parser::Parser::new(tokens, text);
                        if let Ok(program) = parser.parse_program() {
                            // Find calls to 'name'
                            collect_calls_to(name, &program, text, uri, &mut incoming);
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": incoming
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "callHierarchy/outgoingCalls" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let item = params.get("item");
                    let uri_str = item
                        .and_then(|i| i.get("uri"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("");
                    let range = item.and_then(|i| i.get("range"));

                    let state = state_clone.lock().unwrap();
                    let mut outgoing = Vec::new();

                    if let Some(text) = state.documents.get(uri_str) {
                        let tokens = pleno_lexer::lex(text).unwrap_or_default();
                        let mut parser = pleno_parser::Parser::new(tokens, text);
                        if let Ok(program) = parser.parse_program() {
                            // Find all calls WITHIN the function at 'range'
                            collect_calls_from(range, &program, text, &mut outgoing, &state);
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": outgoing
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            _ => {
                // Ignore other methods or respond null if id present
                if value.get("id").is_some() {
                    let id = value.get("id");
                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": Value::Null
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
        }
    }
    Ok(())
}

// --- Helpers ---

fn extract_symbols(program: &Program, file_path: &Path) -> Vec<Symbol> {
    let mut symbols = Vec::new();

    for stmt in &program.statements {
        match &stmt.kind {
            StmtKind::Fn {
                name, params: _, ..
            } => {
                let detail = format!("fn {}(...)", name);
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Function,
                    span: stmt.span,
                    detail: Some(detail),
                    file_path: file_path.to_path_buf(),
                });
            }
            StmtKind::Class {
                name,
                methods,
                fields,
                ..
            } => {
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Class,
                    span: stmt.span,
                    detail: Some(format!("class {}", name)),
                    file_path: file_path.to_path_buf(),
                });

                // Index methods
                for method in methods {
                    let detail = format!("method {}.{}(...)", name, method.name);
                    symbols.push(Symbol {
                        name: method.name.clone(),
                        kind: SymbolKind::Method,
                        span: method.body.span, // Use body span as fallback
                        detail: Some(detail),
                        file_path: file_path.to_path_buf(),
                    });
                }

                // Index fields
                for field in fields {
                    symbols.push(Symbol {
                        name: field.name.clone(),
                        kind: SymbolKind::Field,
                        span: stmt.span, // Fallback to class span as FieldDecl lacks span
                        detail: Some(format!("field {}.{}", name, field.name)),
                        file_path: file_path.to_path_buf(),
                    });
                }
            }
            StmtKind::Interface { name, .. } => {
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Interface,
                    span: stmt.span,
                    detail: Some(format!("interface {}", name)),
                    file_path: file_path.to_path_buf(),
                });
            }
            StmtKind::Let { name, ty, .. } => {
                let t = ty.clone().unwrap_or_else(|| "Any".to_string());
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Variable,
                    span: stmt.span,
                    detail: Some(format!("let {}: {}", name, t)),
                    file_path: file_path.to_path_buf(),
                });
            }
            _ => {}
        }
    }
    symbols
}

fn read_message<R: BufRead + Read>(reader: &mut R) -> io::Result<Option<String>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        if line == "\r\n" {
            break;
        }
        let lower = line.to_lowercase();
        if lower.starts_with("content-length:")
            && let Some(val) = lower.strip_prefix("content-length:")
            && let Ok(len) = val.trim().parse::<usize>()
        {
            content_length = Some(len);
        }
    }

    let len = match content_length {
        Some(len) => len,
        None => return Ok(None),
    };

    let mut buffer = vec![0u8; len];
    reader.read_exact(&mut buffer)?;
    Ok(Some(String::from_utf8_lossy(&buffer).to_string()))
}

fn write_message<W: Write>(writer: &mut W, value: &Value) -> io::Result<()> {
    let text = value.to_string();
    write!(writer, "Content-Length: {}\r\n\r\n{}", text.len(), text)?;
    writer.flush()
}

fn span_to_range(source: &str, start: usize, end: usize) -> (Value, Value) {
    let (start_line, start_col) = offset_to_position(source, start);
    let (end_line, end_col) = offset_to_position(source, end);
    (
        json!({"line": start_line, "character": start_col}),
        json!({"line": end_line, "character": end_col}),
    )
}

fn offset_to_position(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut current = 0usize;

    for ch in source.chars() {
        if current >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1; // Simplification, should handle utf16/utf8 diffs for LSP
        }
        current += ch.len_utf8();
    }
    (line, col)
}

fn collect_semantic_tokens(program: &Program, source: &str) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();

    // 1. Collect from comments
    for comment in &program.comments {
        let (line, col) = offset_to_position(source, comment.span.start);
        tokens.push(SemanticToken {
            line,
            char: col,
            length: (comment.span.end - comment.span.start) as u32,
            token_type: 0, // comment
            token_modifiers: 0,
        });
    }

    // 2. Traversal
    for stmt in &program.statements {
        collect_from_stmt(stmt, source, &mut tokens);
    }

    tokens
}

fn collect_from_stmt(stmt: &pleno_ast::Stmt, source: &str, tokens: &mut Vec<SemanticToken>) {
    use pleno_ast::StmtKind;
    match &stmt.kind {
        StmtKind::Fn {
            name: _,
            params,
            body,
            ..
        } => {
            // Function name
            add_token_from_span(stmt.span, source, 6, 1, tokens); // function, declaration
            for param in params {
                add_token_from_span(param.span, source, 5, 1, tokens); // parameter, declaration
            }
            collect_from_stmt(body, source, tokens);
        }
        StmtKind::Class {
            name: _,
            fields,
            methods,
            nested_classes,
            ..
        } => {
            add_token_from_span(stmt.span, source, 8, 1, tokens); // class, declaration
            for field in fields {
                add_token_from_span(field.span, source, 10, 1, tokens); // property, declaration
            }
            for method in methods {
                add_token_from_span(method.span, source, 7, 1, tokens); // method, declaration
            }
            for nested in nested_classes {
                collect_from_stmt(nested, source, tokens);
            }
        }
        StmtKind::Let {
            name: _,
            ty: _,
            value,
            ..
        } => {
            add_token_from_span(stmt.span, source, 4, 1, tokens); // variable, declaration
            if let Some(v) = value {
                collect_from_expr(v, source, tokens);
            }
        }
        StmtKind::Block(stmts) => {
            for s in stmts {
                collect_from_stmt(s, source, tokens);
            }
        }
        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_from_expr(condition, source, tokens);
            collect_from_stmt(then_branch, source, tokens);
            if let Some(eb) = else_branch {
                collect_from_stmt(eb, source, tokens);
            }
        }
        StmtKind::While { condition, body } => {
            collect_from_expr(condition, source, tokens);
            collect_from_stmt(body, source, tokens);
        }
        StmtKind::Expr(expr) => {
            collect_from_expr(expr, source, tokens);
        }
        _ => {}
    }
}

fn collect_from_expr(expr: &pleno_ast::Expr, source: &str, tokens: &mut Vec<SemanticToken>) {
    use pleno_ast::ExprKind;
    match &expr.kind {
        ExprKind::Ident(_) => {
            add_token_from_span(expr.span, source, 4, 0, tokens); // variable
        }
        ExprKind::Literal(lit) => {
            use pleno_ast::Literal;
            match lit {
                Literal::String(_) => add_token_from_span(expr.span, source, 1, 0, tokens), // string
                Literal::Int(_) | Literal::Float(_) => {
                    add_token_from_span(expr.span, source, 3, 0, tokens)
                } // number
                _ => {}
            }
        }
        ExprKind::Call { callee, args } => {
            collect_from_expr(callee, source, tokens);
            for arg in args {
                match arg {
                    pleno_ast::CallArg::Positional(e) => collect_from_expr(e, source, tokens),
                    pleno_ast::CallArg::Named(_, e) => collect_from_expr(e, source, tokens),
                    _ => {}
                }
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_from_expr(left, source, tokens);
            collect_from_expr(right, source, tokens);
        }
        ExprKind::Unary { expr, .. } => {
            collect_from_expr(expr, source, tokens);
        }
        ExprKind::Block(stmts) => {
            for s in stmts {
                collect_from_stmt(s, source, tokens);
            }
        }
        _ => {}
    }
}

fn add_token_from_span(
    span: Span,
    source: &str,
    token_type: u32,
    modifiers: u32,
    tokens: &mut Vec<SemanticToken>,
) {
    let (line, col) = offset_to_position(source, span.start);
    let len = (span.end - span.start) as u32;
    if len > 0 {
        tokens.push(SemanticToken {
            line,
            char: col,
            length: len,
            token_type,
            token_modifiers: modifiers,
        });
    }
}

fn get_word_at_position(source: &str, line: u32, character: u32) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let l = lines.get(line as usize)?;
    let chars: Vec<char> = l.chars().collect();
    if character as usize >= chars.len() {
        return None;
    }

    // Scan back
    let mut start = character as usize;
    while start > 0 {
        let c = chars[start - 1];
        if c.is_alphanumeric() || c == '_' {
            start -= 1;
        } else {
            break;
        }
    }

    // Scan forward
    let mut end = character as usize;
    while end < chars.len() {
        let c = chars[end];
        if c.is_alphanumeric() || c == '_' {
            end += 1;
        } else {
            break;
        }
    }

    if start < end {
        Some(chars[start..end].iter().collect())
    } else {
        None
    }
}

// Very basic URI to Path conversion (Windows/Unix support)
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    if uri.starts_with("file://") {
        if cfg!(windows) {
            // file:///c%3A/foo/bar -> c:/foo/bar
            // Simple hack for now, real implementation needs url decoding
            let part = uri.trim_start_matches("file:///");
            // Replace / with \ ? Windows can handle /, but C%3A needs handling
            // Basic unescape
            let decoded = part.replace("%3A", ":").replace("%20", " ");
            Some(PathBuf::from(decoded))
        } else {
            Some(PathBuf::from(uri.trim_start_matches("file://")))
        }
    } else {
        None
    }
}

fn collect_calls_to(
    name: &str,
    program: &Program,
    source: &str,
    uri: &str,
    incoming: &mut Vec<Value>,
) {
    for stmt in &program.statements {
        collect_calls_to_in_stmt(name, stmt, source, uri, incoming);
    }
}

fn collect_calls_to_in_stmt(
    name: &str,
    stmt: &pleno_ast::Stmt,
    source: &str,
    uri: &str,
    incoming: &mut Vec<Value>,
) {
    use pleno_ast::StmtKind;
    match &stmt.kind {
        StmtKind::Fn { body, .. } => collect_calls_to_in_stmt(name, body, source, uri, incoming),
        StmtKind::Block(stmts) => {
            for s in stmts {
                collect_calls_to_in_stmt(name, s, source, uri, incoming);
            }
        }
        StmtKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_calls_to_in_expr(name, condition, source, uri, incoming);
            collect_calls_to_in_stmt(name, then_branch, source, uri, incoming);
            if let Some(eb) = else_branch {
                collect_calls_to_in_stmt(name, eb, source, uri, incoming);
            }
        }
        StmtKind::While { condition, body } => {
            collect_calls_to_in_expr(name, condition, source, uri, incoming);
            collect_calls_to_in_stmt(name, body, source, uri, incoming);
        }
        StmtKind::Expr(expr) => collect_calls_to_in_expr(name, expr, source, uri, incoming),
        _ => {}
    }
}

fn collect_calls_to_in_expr(
    name: &str,
    expr: &pleno_ast::Expr,
    source: &str,
    uri: &str,
    incoming: &mut Vec<Value>,
) {
    use pleno_ast::ExprKind;
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            if let ExprKind::Ident(ident) = &callee.kind
                && ident == name
            {
                let (start, end) = span_to_range(source, expr.span.start, expr.span.end);
                incoming.push(json!({
                    "from": {
                        "name": "Unknown Caller", // In a real LSP we'd find the enclosing function
                        "kind": 12,
                        "uri": uri,
                        "range": { "start": start, "end": end },
                        "selectionRange": { "start": start, "end": end }
                    },
                    "fromRanges": [{ "start": start, "end": end }]
                }));
            }
            collect_calls_to_in_expr(name, callee, source, uri, incoming);
            for arg in args {
                match arg {
                    pleno_ast::CallArg::Positional(e) => {
                        collect_calls_to_in_expr(name, e, source, uri, incoming)
                    }
                    pleno_ast::CallArg::Named(_, e) => {
                        collect_calls_to_in_expr(name, e, source, uri, incoming)
                    }
                    _ => {}
                }
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_calls_to_in_expr(name, left, source, uri, incoming);
            collect_calls_to_in_expr(name, right, source, uri, incoming);
        }
        ExprKind::Unary { expr: e, .. } => collect_calls_to_in_expr(name, e, source, uri, incoming),
        ExprKind::Block(stmts) => {
            for s in stmts {
                collect_calls_to_in_stmt(name, s, source, uri, incoming);
            }
        }
        _ => {}
    }
}

fn collect_calls_from(
    _range: Option<&Value>,
    program: &Program,
    source: &str,
    outgoing: &mut Vec<Value>,
    state: &ServerState,
) {
    // For now, simplify and find all calls in the program
    // Ideally we filter by the function at _range
    for stmt in &program.statements {
        collect_calls_from_in_stmt(stmt, source, outgoing, state);
    }
}

fn collect_calls_from_in_stmt(
    stmt: &pleno_ast::Stmt,
    source: &str,
    outgoing: &mut Vec<Value>,
    state: &ServerState,
) {
    use pleno_ast::StmtKind;
    match &stmt.kind {
        StmtKind::Fn { body, .. } => collect_calls_from_in_stmt(body, source, outgoing, state),
        StmtKind::Block(stmts) => {
            for s in stmts {
                collect_calls_from_in_stmt(s, source, outgoing, state);
            }
        }
        StmtKind::Expr(expr) => collect_calls_from_in_expr(expr, source, outgoing, state),
        _ => {}
    }
}

fn collect_calls_from_in_expr(
    expr: &pleno_ast::Expr,
    source: &str,
    outgoing: &mut Vec<Value>,
    state: &ServerState,
) {
    use pleno_ast::ExprKind;
    if let ExprKind::Call { callee, .. } = &expr.kind
        && let ExprKind::Ident(name) = &callee.kind
    {
        // Find definition of 'name' to get its range
        let mut target_uri = None;

        for (uri, symbols) in &state.index {
            if let Some(_sym) = symbols.iter().find(|s| s.name == *name) {
                target_uri = Some(uri.clone());
                break;
            }
        }

        if let Some(uri) = target_uri {
            let (start, end) = span_to_range(source, expr.span.start, expr.span.end);
            outgoing.push(json!({
                "to": {
                    "name": name,
                    "kind": 12,
                    "uri": uri,
                    "range": { "start": start, "end": end },
                    "selectionRange": { "start": start, "end": end }
                },
                "fromRanges": [{ "start": start, "end": end }]
            }));
        }
    }
}
