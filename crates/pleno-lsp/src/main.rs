use serde_json::{Value, json};
use std::io::{self, BufRead, Read, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut stdout = io::stdout();
    let mut documents: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    while let Some(message) = read_message(&mut reader)? {
        let value: Value = match serde_json::from_str(&message) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let method = value.get("method").and_then(|v| v.as_str());
        match method {
            Some("initialize") => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "textDocumentSync": 1,
                            "definitionProvider": true,
                            "hoverProvider": true
                        }
                    }
                });
                write_message(&mut stdout, &response)?;
            }
            Some("textDocument/didOpen") => {
                if let Some(text_document) = value.get("params").and_then(|p| p.get("textDocument"))
                {
                    let uri = text_document
                        .get("uri")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let text = text_document
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    documents.insert(uri.to_string(), text.to_string());
                    let diagnostics = compute_diagnostics(text);
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": {
                            "uri": uri,
                            "diagnostics": diagnostics
                        }
                    });
                    write_message(&mut stdout, &notification)?;
                }
            }
            Some("textDocument/didChange") => {
                if let Some(params) = value.get("params")
                    && let Some(uri) = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri"))
                        .and_then(|v| v.as_str())
                    && let Some(content_changes) =
                        params.get("contentChanges").and_then(|c| c.as_array())
                    && let Some(change) = content_changes.first()
                    && let Some(text) = change.get("text").and_then(|v| v.as_str())
                {
                    documents.insert(uri.to_string(), text.to_string());
                    let diagnostics = compute_diagnostics(text);
                    let notification = json!({
                        "jsonrpc": "2.0",
                        "method": "textDocument/publishDiagnostics",
                        "params": {
                            "uri": uri,
                            "diagnostics": diagnostics
                        }
                    });
                    write_message(&mut stdout, &notification)?;
                }
            }
            Some("textDocument/definition") => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let position = params.get("position");

                    let mut result = Value::Null;
                    if let (Some(text), Some(pos)) = (documents.get(uri), position)
                        && let Some(location) = find_definition(text, pos)
                    {
                        result = json!({
                            "uri": uri,
                            "range": location
                        });
                    }

                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    });
                    write_message(&mut stdout, &response)?;
                }
            }
            Some("textDocument/hover") => {
                let id = value.get("id").cloned().unwrap_or(Value::Null);
                if let Some(params) = value.get("params") {
                    let uri = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let position = params.get("position");

                    let mut result = Value::Null;
                    if let (Some(text), Some(pos)) = (documents.get(uri), position)
                        && let Some(hover) = compute_hover(text, pos)
                    {
                        result = json!({
                            "contents": hover
                        });
                    }

                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    });
                    write_message(&mut stdout, &response)?;
                }
            }
            _ => {
                if value.get("id").is_some() {
                    let id = value.get("id").cloned().unwrap_or(Value::Null);
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": Value::Null
                    });
                    write_message(&mut stdout, &response)?;
                }
            }
        }
    }

    Ok(())
}

fn compute_diagnostics(source: &str) -> Vec<Value> {
    let tokens = match pleno_lexer::lex(source) {
        Ok(tokens) => tokens,
        Err(err) => {
            let (start, end) = span_to_range(source, err.span.start, err.span.end);
            return vec![json!({
                "range": { "start": start, "end": end },
                "severity": 1,
                "message": err.message
            })];
        }
    };

    let mut parser = pleno_parser::Parser::new(tokens);
    if let Err(err) = parser.parse_program() {
        let (start, end) = span_to_range(source, err.span.start, err.span.end);
        return vec![json!({
            "range": { "start": start, "end": end },
            "severity": 1,
            "message": err.message
        })];
    }

    Vec::new()
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
            col += 1;
        }
        current += ch.len_utf8();
    }

    (line, col)
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
        if let Some(value) = line.strip_prefix("Content-Length:")
            && let Ok(len) = value.trim().parse::<usize>()
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

fn find_definition(source: &str, position: &Value) -> Option<Value> {
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;

    let word = get_word_at_position(source, line, character)?;

    // Simple search for "fn word" or "class word" or "let word"
    let lines: Vec<&str> = source.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        if l.contains(&format!("fn {}", word))
            || l.contains(&format!("class {}", word))
            || l.contains(&format!("let {}", word))
        {
            let col = l.find(&word).unwrap_or(0);
            return Some(json!({
                "start": { "line": i, "character": col },
                "end": { "line": i, "character": col + word.len() }
            }));
        }
    }
    None
}

fn compute_hover(source: &str, position: &Value) -> Option<String> {
    let line = position.get("line")?.as_u64()? as u32;
    let character = position.get("character")?.as_u64()? as u32;
    let word = get_word_at_position(source, line, character)?;
    Some(format!("**{}** (Pleno Symbol)", word))
}

fn get_word_at_position(source: &str, line: u32, character: u32) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let l = lines.get(line as usize)?;
    let chars: Vec<char> = l.chars().collect();
    if character >= chars.len() as u32 {
        return None;
    }

    let mut start = character as usize;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }

    let mut end = character as usize;
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
        end += 1;
    }

    if start < end {
        Some(chars[start..end].iter().collect())
    } else {
        None
    }
}
