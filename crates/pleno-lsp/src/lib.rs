use pleno_ast::{Program, Span, StmtKind};
// use pleno_lexer::Token; // Unused
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

        // Parsing
        let mut parser = pleno_parser::Parser::new(tokens);
        match parser.parse_program() {
            Ok(program) => {
                // Index symbols
                if let Some(path) = uri_to_path(&uri) {
                    let symbols = extract_symbols(&program, &path);
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
                            "hoverProvider": true
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
