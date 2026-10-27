use lint::LintSettings;
use pleno_ast::{Program, Span, StmtKind};
use pleno_formatter::{FormatOptions, format_code};
use pleno_lexer::lex;
use pleno_parser::Parser;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod lint;
mod navigation;
mod semantic;

// --- Data Structures ---

#[derive(Debug, Clone)]
struct Symbol {
    name: String,
    kind: SymbolKind,
    span: Span,
    detail: Option<String>,
    children: Vec<Symbol>,
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
    // Map URI -> Parsed Program (AST)
    programs: HashMap<String, Program>,
    // Map URI -> Parsed Symbols
    index: HashMap<String, Vec<Symbol>>,
    root_uri: Option<String>,
    lint: LintSettings,
}

impl ServerState {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            programs: HashMap::new(),
            index: HashMap::new(),
            root_uri: None,
            lint: LintSettings::default(),
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
                    "source": "pleno",
                    "code": "syntax-error",
                    "message": e.message
                }));
                return diagnostics;
            }
        };

        // Linting
        diagnostics.extend(lint::lint_diagnostics(&text, &uri, &self.lint));

        // Parsing
        let mut parser = pleno_parser::Parser::new(tokens, &text);
        match parser.parse_program() {
            Ok(mut program) => {
                let path = uri_to_path(&uri);
                // The static checks `pleno check` runs: argument counts, unknown
                // methods, names a `.pln` file does not export, values that do
                // not match their annotation.
                let found = pleno_compiler::check::check_program_in(&program, path.as_deref());
                for found in found {
                    let (start, end) = span_to_range(&text, found.span.start, found.span.end);
                    let severity = match found.severity {
                        pleno_compiler::check::Severity::Error => 1,
                        pleno_compiler::check::Severity::Warning => 2,
                    };
                    diagnostics.push(json!({
                        "range": { "start": start, "end": end },
                        "severity": severity,
                        "source": "pleno",
                        "code": "check",
                        "message": found.message
                    }));
                }
                let compile_error = compile_diagnostic(&text, &mut program);
                let mut duplicates = Vec::new();

                // Index symbols
                if path.is_some() {
                    let symbols = extract_symbols(&program);

                    // Check for re-definitions in this file
                    let mut seen_names: std::collections::HashMap<String, Symbol> =
                        std::collections::HashMap::new();
                    for sym in &symbols {
                        if let Some(original) = seen_names.get(&sym.name) {
                            duplicates.push(format!("Local '{}' already defined", sym.name));
                            let (start, end) = span_to_range(&text, sym.span.start, sym.span.end);
                            let (orig_start, orig_end) =
                                span_to_range(&text, original.span.start, original.span.end);
                            diagnostics.push(json!({
                                "range": { "start": start, "end": end },
                                "severity": 1,
                                "source": "pleno",
                                "code": "duplicate-definition",
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

                    self.index.insert(uri.clone(), symbols);
                }
                // The redefinition the compiler stops at is reported above.
                if let Some(error) = compile_error
                    && !duplicates.iter().any(|d| error["message"] == d.as_str())
                {
                    diagnostics.push(error);
                }
                self.programs.insert(uri, program);
            }
            Err(e) => {
                let (start, end) = span_to_range(&text, e.span.start, e.span.end);
                diagnostics.push(json!({
                    "range": { "start": start, "end": end },
                    "severity": 1,
                    "source": "pleno",
                    "code": "parse-error",
                    "message": e.message
                }));
            }
        }

        diagnostics
    }
}

/// The first compile error in `program`, such as `cannot assign to immutable
/// binding`, as `pleno check` finds it: all code is compiled, unused functions
/// too, and a file that exports names is compiled as a module. Compiling this
/// way leaves `program` as it was.
fn compile_diagnostic(text: &str, program: &mut Program) -> Option<Value> {
    let compile = std::panic::AssertUnwindSafe(|| {
        pleno_compiler::Compiler::new(text.to_string())
            .keeping_unused_code()
            .compile_file(program)
            .err()
    });
    // A compiler bug must not take the server down with it.
    let err = std::panic::catch_unwind(compile).ok()??;
    let (start, end) = err
        .diagnostic
        .labels
        .first()
        .map(|label| (label.span.start, label.span.end))
        .unwrap_or((0, 0));
    let (start, end) = span_to_range(text, start, end);
    Some(json!({
        "range": { "start": start, "end": end },
        "severity": 1,
        "source": "pleno",
        "code": "compile-error",
        "message": err.diagnostic.message
    }))
}

// --- Main Loop ---

pub fn start() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        println!("Pleno Language Server (pleno-lsp)");
        println!();
        println!("Usage: pleno-lsp [options]");
        println!();
        println!("Options:");
        println!("  -h, --help     Show this help message");
        println!("  -v, --version  Show version information");
        return Ok(());
    }

    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("pleno-lsp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

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
                if let Some(params) = value.get("params") {
                    let first_folder = params
                        .get("workspaceFolders")
                        .and_then(|f| f.get(0))
                        .and_then(|f| f.get("uri"));
                    if let Some(root) = params
                        .get("rootUri")
                        .filter(|v| !v.is_null())
                        .or(first_folder)
                        .and_then(|v| v.as_str())
                    {
                        state.root_uri = Some(root.to_string());
                    }
                    if let Some(lint) = params
                        .get("initializationOptions")
                        .and_then(|o| o.get("lint"))
                    {
                        state.lint = LintSettings::from_json(lint);
                    }
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
                            "callHierarchyProvider": true,
                            "referencesProvider": true,
                            "renameProvider": { "prepareProvider": true },
                            "documentHighlightProvider": true,
                            "workspaceSymbolProvider": true,
                            "selectionRangeProvider": true,
                            "documentRangeFormattingProvider": true,
                            "signatureHelpProvider": {
                                "triggerCharacters": ["(", ","]
                            }
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
                    if let Some(text) = state.documents.get(uri_str).cloned()
                        && let Some(pos_val) = pos_json
                    {
                        let line = pos_val.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                        let char = pos_val
                            .get("character")
                            .and_then(|c| c.as_u64())
                            .unwrap_or(0) as u32;

                        if let Some(offset) = position_to_offset(&text, line, char)
                            && let Ok(tokens) = lex(&text)
                        {
                            let mut parser = Parser::new(tokens, &text);
                            if let Ok(program) = parser.parse_program()
                                && let Some(span) =
                                    semantic::usage_finder::find_declaration(&program, offset)
                            {
                                let (start, end) = span_to_range(&text, span.start, span.end);
                                result = json!({
                                    "uri": uri_str,
                                    "range": { "start": start, "end": end }
                                });
                            }
                        }

                        // Fallback to naive search if semantic search failed (for cross-file or non-local symbols)
                        if result.is_null()
                            && let Some(word) = get_word_at_position(&text, line, char)
                        {
                            for (idx_uri, symbols) in &state.index {
                                if let Some(sym) = symbols.iter().find(|s| s.name == word) {
                                    let file_content = if let Some(c) = state.documents.get(idx_uri)
                                    {
                                        c.clone()
                                    } else if let Some(p) = uri_to_path(idx_uri) {
                                        fs::read_to_string(p).unwrap_or_default()
                                    } else {
                                        String::new()
                                    };

                                    if !file_content.is_empty() {
                                        let (start, end) = span_to_range(
                                            &file_content,
                                            sym.span.start,
                                            sym.span.end,
                                        );
                                        result = json!({
                                            "uri": idx_uri,
                                            "range": { "start": start, "end": end }
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
            "textDocument/references" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let pos_json = params.get("position");

                    let mut references: Vec<Value> = Vec::new();

                    let state = state_clone.lock().unwrap();
                    if let Some(text) = state.documents.get(uri_str).cloned()
                        && let Some(program) = state.programs.get(uri_str)
                        && let Some(pos_val) = pos_json
                    {
                        let line = pos_val.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                        let char = pos_val
                            .get("character")
                            .and_then(|c| c.as_u64())
                            .unwrap_or(0) as u32;

                        if let Some(offset) = position_to_offset(&text, line, char) {
                            // Also when the cursor is just after the name.
                            let offset =
                                navigation::word_span(&text, offset).map_or(offset, |w| w.start);
                            let spans = semantic::usage_finder::find_usages(program, offset);
                            for span in spans {
                                let (start, end) = span_to_range(&text, span.start, span.end);
                                references.push(json!({
                                    "uri": uri_str,
                                    "range": {
                                        "start": start,
                                        "end": end
                                    }
                                }));
                            }
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": references
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/rename" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri_str = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri").and_then(|v| v.as_str()))
                        .unwrap_or_default();
                    let pos_json = params.get("position");
                    let new_name = params
                        .get("newName")
                        .and_then(|n| n.as_str())
                        .unwrap_or_default();

                    let mut changes: HashMap<String, Vec<Value>> = HashMap::new();

                    let state = state_clone.lock().unwrap();
                    if let Some(text) = state.documents.get(uri_str).cloned()
                        && let Some(program) = state.programs.get(uri_str)
                        && let Some(pos_val) = pos_json
                        && !new_name.is_empty()
                    {
                        let line = pos_val.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                        let char = pos_val
                            .get("character")
                            .and_then(|c| c.as_u64())
                            .unwrap_or(0) as u32;

                        if let Some(offset) = position_to_offset(&text, line, char) {
                            // Also when the cursor is just after the name.
                            let offset =
                                navigation::word_span(&text, offset).map_or(offset, |w| w.start);
                            let spans = semantic::usage_finder::find_usages(program, offset);
                            let mut edits = Vec::new();
                            for span in spans {
                                let (start, end) = span_to_range(&text, span.start, span.end);
                                edits.push(json!({
                                    "range": {
                                        "start": start,
                                        "end": end
                                    },
                                    "newText": new_name
                                }));
                            }
                            if !edits.is_empty() {
                                changes.insert(uri_str.to_string(), edits);
                            }
                        }
                    }

                    let resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "changes": changes
                        }
                    });
                    write_message(&mut stdout, &resp)?;
                }
            }
            "textDocument/signatureHelp" => {
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

                        if let Some((name, active_param)) = get_signature_context(&text, line, char)
                        {
                            // Find definition in index to get the signature string
                            for symbols in state.index.values() {
                                if let Some(sym) = symbols.iter().find(|s| s.name == name)
                                    && let Some(detail) = &sym.detail
                                {
                                    // Found it!
                                    result = json!({
                                        "signatures": [
                                            {
                                                "label": detail,
                                                "parameters": [] // Optional: parse detail for parameter ranges
                                            }
                                        ],
                                        "activeSignature": 0,
                                        "activeParameter": active_param
                                    });
                                    break;
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
                            let prefix = &current_line[..utf16_to_byte(current_line, character)];

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
                                    "with",
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
                                ];

                                for kw in keywords {
                                    items.push(json!({
                                        "label": kw,
                                        "kind": 14,
                                        "detail": "Keyword"
                                    }));
                                }

                                // The globals the standard library defines.
                                let mut builtins: Vec<&String> =
                                    pleno_linter::builtin_globals().iter().collect();
                                builtins.sort();
                                for name in builtins {
                                    let is_type = name.starts_with(char::is_uppercase);
                                    items.push(json!({
                                        "label": name,
                                        "kind": if is_type { 7 } else { 3 },
                                        "detail": "Built-in"
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
                            symbols_out.push(symbol_to_json(sym, text, uri_str));
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

                    // Fixes edit the current text, so they work from a fresh parse
                    // rather than the last one that succeeded.
                    let state = state_clone.lock().unwrap();
                    let text = state.documents.get(uri_str).cloned().unwrap_or_default();
                    drop(state);
                    let program = lex(&text)
                        .ok()
                        .and_then(|tokens| Parser::new(tokens, &text).parse_program().ok());

                    if let Some(diags) = diagnostics {
                        for diag in diags {
                            let message =
                                diag.get("message").and_then(|m| m.as_str()).unwrap_or("");
                            let range = diag.get("range").cloned().unwrap_or(Value::Null);

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

                            actions.extend(lint::quick_fixes(
                                diag,
                                &text,
                                program.as_ref(),
                                uri_str,
                            ));
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

                    // A script runs from its first line, so the lenses run or debug
                    // the whole file; they sit above its first statement.
                    let tokens = pleno_lexer::lex(text).unwrap_or_default();
                    let mut parser = pleno_parser::Parser::new(tokens, text);
                    if let Ok(program) = parser.parse_program()
                        && let Some(first) = program.statements.first()
                    {
                        let (start, _) = span_to_range(text, first.span.start, first.span.end);
                        for (title, command) in [
                            ("$(play) Run File", "pleno.runFile"),
                            ("$(debug-alt) Debug File", "pleno.debugFile"),
                        ] {
                            lenses.push(json!({
                                "range": { "start": start, "end": start },
                                "command": {
                                    "title": title,
                                    "command": command,
                                    "arguments": [uri_str]
                                }
                            }));
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
            "textDocument/documentHighlight" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                let (uri_str, line, char) = request_position(&value);
                let mut highlights = Vec::new();

                let state = state_clone.lock().unwrap();
                if let Some(text) = state.documents.get(uri_str)
                    && let Some(program) = state.programs.get(uri_str)
                    && let Some(offset) = position_to_offset(text, line, char)
                {
                    highlights = navigation::document_highlights(program, text, offset);
                }

                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": highlights
                });
                write_message(&mut stdout, &resp)?;
            }
            "textDocument/prepareRename" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                let (uri_str, line, char) = request_position(&value);

                let state = state_clone.lock().unwrap();
                let result = match (state.documents.get(uri_str), state.programs.get(uri_str)) {
                    (Some(text), Some(program)) => position_to_offset(text, line, char)
                        .ok_or_else(|| "Only names can be renamed.".to_string())
                        .and_then(|offset| navigation::prepare_rename(program, text, offset)),
                    _ => Err("The file has not been parsed yet.".to_string()),
                };

                // An error carries the reason the editor shows to the user.
                let resp = match result {
                    Ok(range) => json!({ "jsonrpc": "2.0", "id": id, "result": range }),
                    Err(message) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32803, "message": message }
                    }),
                };
                write_message(&mut stdout, &resp)?;
            }
            "textDocument/selectionRange" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                let (uri_str, _, _) = request_position(&value);
                let positions = value
                    .get("params")
                    .and_then(|p| p.get("positions"))
                    .and_then(|p| p.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut ranges = Vec::new();

                let state = state_clone.lock().unwrap();
                if let Some(text) = state.documents.get(uri_str)
                    && let Some(program) = state.programs.get(uri_str)
                {
                    for pos in &positions {
                        let line = pos.get("line").and_then(|l| l.as_u64()).unwrap_or(0) as u32;
                        let char =
                            pos.get("character").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
                        let offset = position_to_offset(text, line, char).unwrap_or(text.len());
                        ranges.push(navigation::selection_range(program, text, offset));
                    }
                }

                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": ranges
                });
                write_message(&mut stdout, &resp)?;
            }
            "workspace/symbol" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                let query = value
                    .get("params")
                    .and_then(|p| p.get("query"))
                    .and_then(|q| q.as_str())
                    .unwrap_or_default();

                let state = state_clone.lock().unwrap();
                let symbols = navigation::workspace_symbols(&state, query);

                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": symbols
                });
                write_message(&mut stdout, &resp)?;
            }
            "textDocument/rangeFormatting" => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                let (uri_str, _, _) = request_position(&value);
                let range = value.get("params").and_then(|p| p.get("range"));
                let position = |key: &str| {
                    let pos = range?.get(key)?;
                    let line = pos.get("line")?.as_u64()? as u32;
                    let char = pos.get("character")?.as_u64()? as u32;
                    Some((line, char))
                };
                let mut edits = Vec::new();

                let state = state_clone.lock().unwrap();
                if let Some(text) = state.documents.get(uri_str)
                    && let (Some(start), Some(end)) = (position("start"), position("end"))
                    && let Some(start) = position_to_offset(text, start.0, start.1)
                    && let Some(end) = position_to_offset(text, end.0, end.1)
                    && let Ok(tokens) = lex(text)
                    && let Ok(program) = Parser::new(tokens, text).parse_program()
                {
                    edits = navigation::format_range(&program, text, start, end);
                }

                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": edits
                });
                write_message(&mut stdout, &resp)?;
            }
            "workspace/didChangeConfiguration" => {
                // VS Code sends `{ settings: { pleno: { lint: ... } } }`.
                let settings = value.get("params").and_then(|p| p.get("settings"));
                let lint = settings
                    .and_then(|s| s.get("pleno"))
                    .or(settings)
                    .and_then(|s| s.get("lint"));
                if let Some(lint) = lint {
                    let mut state = state_clone.lock().unwrap();
                    let lint = LintSettings::from_json(lint);
                    if state.lint != lint {
                        state.lint = lint;
                        // Re-check the open documents with the new rules.
                        let mut documents: Vec<(String, String)> = state
                            .documents
                            .iter()
                            .map(|(uri, text)| (uri.clone(), text.clone()))
                            .collect();
                        documents.sort();
                        for (uri, text) in documents {
                            let diags = state.update_document(uri.clone(), text);
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

fn extract_symbols(program: &Program) -> Vec<Symbol> {
    let mut symbols = Vec::new();

    for stmt in &program.statements {
        // `export fn f` declares `f` like `fn f` does.
        let stmt = match &stmt.kind {
            StmtKind::Export(inner) => inner,
            _ => stmt,
        };
        match &stmt.kind {
            StmtKind::Fn {
                name,
                params,
                name_span,
                ..
            } => {
                let params_str = params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.ty.as_deref().unwrap_or("Any")))
                    .collect::<Vec<_>>()
                    .join(", ");
                let detail = format!("fn {}({})", name, params_str);
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Function,
                    span: *name_span,
                    detail: Some(detail),
                    children: vec![],
                });
            }
            StmtKind::Class {
                name,
                methods,
                fields,
                name_span,
                ..
            } => {
                let mut class_children = Vec::new();

                // Index methods
                for method in methods {
                    let params_str = method
                        .params
                        .iter()
                        .map(|p| format!("{}: {}", p.name, p.ty.as_deref().unwrap_or("Any")))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let detail = format!("method {}({})", method.name, params_str);
                    class_children.push(Symbol {
                        name: method.name.clone(),
                        kind: SymbolKind::Method,
                        span: method.name_span,
                        detail: Some(detail),
                        children: vec![],
                    });
                }

                // Index fields
                for field in fields {
                    class_children.push(Symbol {
                        name: field.name.clone(),
                        kind: SymbolKind::Field,
                        span: field.name_span, // Assuming FieldDecl has name_span now (verified in parser)
                        detail: Some(format!("field {}", field.name)),
                        children: vec![],
                    });
                }

                symbols.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Class,
                    span: *name_span,
                    detail: Some(format!("class {}", name)),
                    children: class_children,
                });
            }
            StmtKind::Interface {
                name, name_span, ..
            } => {
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Interface,
                    span: *name_span,
                    detail: Some(format!("interface {}", name)),
                    children: vec![],
                });
            }
            StmtKind::Let {
                name,
                ty,
                name_span,
                ..
            } => {
                let t = ty.clone().unwrap_or_else(|| "Any".to_string());
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Variable,
                    span: *name_span,
                    detail: Some(format!("let {}: {}", name, t)),
                    children: vec![],
                });
            }
            StmtKind::LetPattern { pattern, .. } => {
                for (name, span) in pattern.bindings() {
                    symbols.push(Symbol {
                        name: name.to_string(),
                        kind: SymbolKind::Variable,
                        span,
                        detail: Some(format!("let {name}")),
                        children: vec![],
                    });
                }
            }
            _ => {}
        }
    }
    symbols
}

/// The document URI and the (line, character) position of a request.
fn request_position(value: &Value) -> (&str, u32, u32) {
    let params = value.get("params");
    let uri = params
        .and_then(|p| p.get("textDocument"))
        .and_then(|t| t.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or_default();
    let position = params.and_then(|p| p.get("position"));
    let get = |key: &str| {
        position
            .and_then(|p| p.get(key))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32
    };
    (uri, get("line"), get("character"))
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
            // LSP positions count UTF-16 code units.
            col += ch.len_utf16() as u32;
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
        StmtKind::LetPattern { pattern, value, .. } => {
            for (_, span) in pattern.bindings() {
                add_token_from_span(span, source, 4, 1, tokens); // variable, declaration
            }
            collect_from_expr(value, source, tokens);
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
        StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            if let Some(pattern) = pattern {
                for (_, span) in pattern.bindings() {
                    add_token_from_span(span, source, 4, 1, tokens); // variable, declaration
                }
            }
            collect_from_expr(iterable, source, tokens);
            collect_from_stmt(body, source, tokens);
        }
        StmtKind::With { resources, body } => {
            for resource in resources {
                add_token_from_span(resource.name_span, source, 4, 1, tokens); // variable, declaration
                collect_from_expr(&resource.value, source, tokens);
            }
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

fn symbol_to_json(sym: &Symbol, text: &str, uri: &str) -> Value {
    let (start, end) = span_to_range(text, sym.span.start, sym.span.end);
    let mut children = Vec::new();
    for child in &sym.children {
        children.push(symbol_to_json(child, text, uri));
    }

    let mut map = serde_json::Map::new();
    map.insert("name".to_string(), json!(sym.name));
    map.insert(
        "location".to_string(),
        json!({
            "uri": uri,
            "range": {
                "start": start,
                "end": end
            }
        }),
    );
    map.insert(
        "kind".to_string(),
        json!(match sym.kind {
            SymbolKind::Function | SymbolKind::Method => 12, // Function
            SymbolKind::Class => 5,                          // Class
            SymbolKind::Variable | SymbolKind::Field => 13,  // Variable
            SymbolKind::Interface => 11,                     // Interface
        }),
    );
    map.insert("range".to_string(), json!({ "start": start, "end": end }));
    map.insert(
        "selectionRange".to_string(),
        json!({ "start": start, "end": end }),
    );
    if let Some(detail) = &sym.detail {
        map.insert("detail".to_string(), json!(detail));
    }
    if !children.is_empty() {
        map.insert("children".to_string(), json!(children));
    }

    Value::Object(map)
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

fn get_signature_context(source: &str, line: u32, character: u32) -> Option<(String, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    if line as usize >= lines.len() {
        return None;
    }
    let current_line = lines[line as usize];
    if character as usize > current_line.len() {
        return None;
    }

    let mut paren_count = 0;
    let mut comma_count = 0;
    let mut found_open_paren = false;
    let mut paren_pos = 0;

    let search_part = &current_line[..character as usize];
    for (i, ch) in search_part.char_indices().rev() {
        match ch {
            ',' if paren_count == 0 => comma_count += 1,
            ')' => paren_count += 1,
            '(' => {
                if paren_count == 0 {
                    found_open_paren = true;
                    paren_pos = i;
                    break;
                } else {
                    paren_count -= 1;
                }
            }
            _ => {}
        }
    }

    if found_open_paren {
        // Find identifier before '('
        let before_paren = &current_line[..paren_pos];
        let trimmed_before = before_paren.trim_end();
        let mut id_start = trimmed_before.len();
        for (i, ch) in trimmed_before.char_indices().rev() {
            if ch.is_alphanumeric() || ch == '_' {
                id_start = i;
            } else {
                break;
            }
        }
        if id_start < trimmed_before.len() {
            let name = trimmed_before[id_start..].to_string();
            return Some((name, comma_count));
        }
    }

    None
}

/// Converts a `file://` URI to a path, decoding percent-escapes and drive
/// letters (`file:///c%3A/My%20Files/a.pln` -> `c:\My Files\a.pln`).
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    url::Url::parse(uri).ok()?.to_file_path().ok()
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

/// Converts an LSP position (line, UTF-16 column) to a byte offset. Works with
/// both `\n` and `\r\n` line endings; a column past the end of the line
/// maps to the line's end.
fn position_to_offset(source: &str, line: u32, character: u32) -> Option<usize> {
    let mut offset = 0;
    for (index, text) in source.split_inclusive('\n').enumerate() {
        if index == line as usize {
            let content = text.trim_end_matches(['\n', '\r']);
            return Some(offset + utf16_to_byte(content, character));
        }
        offset += text.len();
    }
    // The position just after a final newline starts an empty last line.
    let line_count = source.split_inclusive('\n').count();
    (line as usize == line_count && (source.is_empty() || source.ends_with('\n')))
        .then_some(source.len())
}

/// The byte index in `line` of an LSP (UTF-16) column, clamped to the line end.
fn utf16_to_byte(line: &str, character: u32) -> usize {
    let mut units = 0;
    for (byte, ch) in line.char_indices() {
        if units >= character as usize {
            return byte;
        }
        units += ch.len_utf16();
    }
    line.len()
}

#[cfg(test)]
mod position_tests {
    use super::*;

    #[test]
    fn positions_handle_crlf_and_utf16() {
        let source = "let a = 1;\r\nlet 😀b = 2;\r\n";
        // Line 1 starts after "let a = 1;\r\n" (12 bytes).
        assert_eq!(position_to_offset(source, 1, 0), Some(12));
        // The emoji is 2 UTF-16 units and 4 bytes: column 6 is just after it.
        assert_eq!(position_to_offset(source, 1, 6), Some(12 + 4 + 4));
        // Columns past the end clamp to the end of the line, before "\r\n".
        assert_eq!(position_to_offset(source, 0, 99), Some(10));
        assert_eq!(position_to_offset(source, 2, 0), Some(source.len()));
        assert_eq!(position_to_offset(source, 3, 0), None);

        assert_eq!(offset_to_position(source, 12 + 4 + 4), (1, 6));
    }

    #[test]
    fn documents_get_static_check_diagnostics() {
        let mut state = ServerState::new();
        let text = "fn greet(name) {}\ngreet(1, 2);\nlet x: Int = \"hi\";\n".to_string();
        let diagnostics = state.update_document("untitled:Untitled-1".to_string(), text);
        let messages: Vec<&str> = diagnostics
            .iter()
            .filter(|d| d["code"] == "check")
            .filter_map(|d| d["message"].as_str())
            .collect();
        assert_eq!(
            messages,
            [
                "greet expects 1 argument, got 2",
                "'x' is declared Int but given a String literal"
            ]
        );
        let first = diagnostics.iter().find(|d| d["code"] == "check").unwrap();
        assert_eq!(first["severity"], 1);
        assert_eq!(first["range"]["start"]["line"], 1);
        assert_eq!(first["range"]["start"]["character"], 0);
    }

    #[test]
    fn uris_are_percent_decoded() {
        let uri = if cfg!(windows) {
            "file:///c%3A/My%20Files/%D0%BF%D1%80%D0%BE%D0%B5%D0%BA%D1%82/a.pln"
        } else {
            "file:///home/My%20Files/%D0%BF%D1%80%D0%BE%D0%B5%D0%BA%D1%82/a.pln"
        };
        let path = uri_to_path(uri).unwrap();
        let text = path.to_string_lossy();
        assert!(text.contains("My Files"), "{text}");
        assert!(text.contains("проект"), "{text}");
        assert!(uri_to_path("untitled:Untitled-1").is_none());
    }
}
