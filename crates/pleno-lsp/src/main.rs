use serde_json::{Value, json};
use std::io::{self, BufRead, Read, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut stdout = io::stdout();

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
                            "textDocumentSync": 1
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
