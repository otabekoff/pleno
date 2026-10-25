//! Formatting every Pleno file in the repository must keep it valid: the output
//! parses whenever the input does, and formatting the output changes nothing.

use pleno_formatter::{FormatOptions, format_code};
use std::path::{Path, PathBuf};

fn parses(source: &str) -> Result<(), String> {
    let tokens = pleno_lexer::lex(source).map_err(|e| e.message)?;
    pleno_parser::Parser::new(tokens, source)
        .parse_program()
        .map(|_| ())
        .map_err(|e| e.message)
}

fn pln_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            pln_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "pln") {
            out.push(path);
        }
    }
}

#[test]
fn formatting_repository_files_keeps_them_valid_and_stable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["tests", "examples", "dev-tests"] {
        pln_files(&root.join(dir), &mut files);
    }
    assert!(files.len() > 100, "found only {} files", files.len());

    let options = FormatOptions::default();
    let mut failures = Vec::new();
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap();
        if parses(&source).is_err() {
            continue; // Files that test syntax errors.
        }
        let name = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        let formatted = match format_code(&source, &options) {
            Ok(formatted) => formatted,
            Err(e) => {
                failures.push(format!("{name}: formatter failed: {e}"));
                continue;
            }
        };
        if let Err(e) = parses(&formatted) {
            failures.push(format!("{name}: formatted code does not parse: {e}"));
            continue;
        }
        match format_code(&formatted, &options) {
            Ok(again) if again == formatted => {}
            Ok(_) => failures.push(format!("{name}: a second format changes the file")),
            Err(e) => failures.push(format!("{name}: second format failed: {e}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
