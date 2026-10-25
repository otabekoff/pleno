use pleno_diagnostics::{Diagnostic, DiagnosticReporter, SourceMap, ToDiagnostic};
use pleno_lexer::{TokenKind, lex};
use pleno_vm::VM;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{CmdKind, Highlighter, MatchingBracketHighlighter};
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;

struct PlenoHelper {
    highlighter: MatchingBracketHighlighter,
}

impl PlenoHelper {
    fn new() -> Self {
        Self {
            highlighter: MatchingBracketHighlighter::new(),
        }
    }
}

impl Helper for PlenoHelper {}

impl Highlighter for PlenoHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let tokens = match lex(line) {
            Ok(t) => t,
            Err(_) => return Cow::Borrowed(line),
        };

        let mut highlighted = String::new();
        let mut last_pos = 0;

        for token in tokens {
            // Append whitespace/ignored parts
            if token.span.start > last_pos {
                highlighted.push_str(&line[last_pos..token.span.start]);
            }

            let color = match &token.kind {
                TokenKind::KeywordLet
                | TokenKind::KeywordMut
                | TokenKind::KeywordConst
                | TokenKind::KeywordFn
                | TokenKind::KeywordReturn
                | TokenKind::KeywordIf
                | TokenKind::KeywordElse
                | TokenKind::KeywordWhile
                | TokenKind::KeywordFor
                | TokenKind::KeywordIn
                | TokenKind::KeywordBreak
                | TokenKind::KeywordContinue
                | TokenKind::KeywordExtends
                | TokenKind::KeywordStatic
                | TokenKind::KeywordPublic
                | TokenKind::KeywordPrivate
                | TokenKind::KeywordProtected
                | TokenKind::KeywordInternal
                | TokenKind::KeywordImport
                | TokenKind::KeywordFrom
                | TokenKind::KeywordAs
                | TokenKind::KeywordExport
                | TokenKind::KeywordModule
                | TokenKind::KeywordClass
                | TokenKind::KeywordThrow
                | TokenKind::KeywordTry
                | TokenKind::KeywordCatch
                | TokenKind::KeywordFinally
                | TokenKind::KeywordMatch
                | TokenKind::KeywordCase
                | TokenKind::KeywordNew
                | TokenKind::KeywordThis
                | TokenKind::KeywordSuper
                | TokenKind::KeywordAsync
                | TokenKind::KeywordAwait
                | TokenKind::KeywordDo
                | TokenKind::KeywordAbstract
                | TokenKind::KeywordInterface
                | TokenKind::KeywordImplements
                | TokenKind::KeywordProperty
                | TokenKind::KeywordGet
                | TokenKind::KeywordSet
                | TokenKind::KeywordDeep
                | TokenKind::KeywordEnum
                | TokenKind::KeywordType
                | TokenKind::KeywordExtend
                | TokenKind::KeywordIs
                | TokenKind::KeywordOperator
                | TokenKind::KeywordMacro
                | TokenKind::KeywordLazy
                | TokenKind::KeywordMove => "\x1b[1;35m", // Magenta Bold
                TokenKind::String(_) | TokenKind::Char(_) => "\x1b[32m", // Green
                TokenKind::Comment(_, _) => "\x1b[90m",                  // Gray
                TokenKind::Int(_) | TokenKind::Float(_) | TokenKind::Bool(_) | TokenKind::Null => {
                    "\x1b[33m"
                } // Yellow
                TokenKind::Ident(_) => {
                    if line[token.span.end..].starts_with('(') {
                        "\x1b[34m" // Blue
                    } else {
                        "\x1b[37m" // White
                    }
                }
                _ => "\x1b[0m",
            };

            highlighted.push_str(color);
            highlighted.push_str(&line[token.span.start..token.span.end]);
            highlighted.push_str("\x1b[0m");

            last_pos = token.span.end;
        }

        if last_pos < line.len() {
            highlighted.push_str(&line[last_pos..]);
        }

        Cow::Owned(highlighted)
    }

    fn highlight_char(&self, line: &str, pos: usize, kind: CmdKind) -> bool {
        self.highlighter.highlight_char(line, pos, kind)
    }
}

impl Completer for PlenoHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let keywords = vec![
            "let",
            "mut",
            "const",
            "fn",
            "class",
            "interface",
            "enum",
            "if",
            "else",
            "while",
            "for",
            "in",
            "break",
            "continue",
            "return",
            "try",
            "catch",
            "finally",
            "throw",
            "import",
            "export",
            "is",
            "as",
            "async",
            "await",
            "pub",
            "static",
            "abstract",
            "module",
            "extend",
            "implements",
            "property",
            "get",
            "set",
            "deep",
            "operator",
            "macro",
            "lazy",
            "move",
        ];

        let builtins = vec![
            "print", "println", "len", "type", "range", "assert", "panic",
        ];

        // Find the word being typed
        let mut start = pos;
        while start > 0 {
            let ch = line.chars().nth(start - 1).unwrap();
            if ch.is_alphanumeric() || ch == '_' {
                start -= 1;
            } else {
                break;
            }
        }

        let word = &line[start..pos];
        if word.is_empty() {
            return Ok((pos, vec![]));
        }

        let mut matches = Vec::new();
        for kw in keywords {
            if kw.starts_with(word) {
                matches.push(Pair {
                    display: kw.to_string(),
                    replacement: kw.to_string(),
                });
            }
        }
        for b in builtins {
            if b.starts_with(word) {
                matches.push(Pair {
                    display: format!("{}()", b),
                    replacement: b.to_string(),
                });
            }
        }

        Ok((start, matches))
    }
}

impl Hinter for PlenoHelper {
    type Hint = String;
}

impl Validator for PlenoHelper {}

use pleno_compiler::Compiler;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, Editor, Result};

fn is_complete(input: &str) -> bool {
    if input.trim().is_empty() {
        return true;
    }

    // Check balance of {}, (), []
    let mut braces = 0;
    let mut parens = 0;
    let mut brackets = 0;
    let mut in_string = false;
    let mut escaped = false;

    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            _ if in_string => {}
            '{' => braces += 1,
            '}' => braces -= 1,
            '(' => parens += 1,
            ')' => parens -= 1,
            '[' => brackets += 1,
            ']' => brackets -= 1,
            '/' => {
                if let Some('/') = chars.peek() {
                    // Single line comment, skip until newline
                    for next_ch in chars.by_ref() {
                        if next_ch == '\n' {
                            break;
                        }
                    }
                } else if let Some('*') = chars.peek() {
                    // Multi-line comment, skip until */
                    chars.next();
                    while let Some(next_ch) = chars.next() {
                        if next_ch == '*' && matches!(chars.peek(), Some('/')) {
                            chars.next();
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if braces > 0 || parens > 0 || brackets > 0 {
        return false;
    }

    // Lex check for unterminated strings/comments
    match lex(input) {
        Ok(_) => true,
        Err(err) => !err.is_incomplete(),
    }
}

pub fn start() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        println!("Pleno Interactive REPL (pleno-repl)");
        println!();
        println!("Usage: pleno-repl [options]");
        println!();
        println!("Options:");
        println!("  -h, --help     Show this help message");
        println!("  -v, --version  Show version information");
        return Ok(());
    }

    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("pleno-repl {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);

    let config = Config::builder()
        .edit_mode(rustyline::EditMode::Emacs)
        .build();
    let mut rl = Editor::<PlenoHelper, FileHistory>::with_config(config)?;
    rl.set_helper(Some(PlenoHelper::new()));

    // Load history if exists
    let history_path = dirs_history_path();
    if let Some(ref path) = history_path
        && rl.load_history(path).is_err()
    {
        // It's okay if history doesn't exist yet, but warn on other errors
        // println!("Warning: Could not load history: {}", e);
    }

    {
        use pleno_common::style::{self, Stream};
        let out = Stream::Stdout;
        println!(
            "{} {}",
            style::gradient(out, "Pleno", &style::BRAND),
            style::dim(out, &format!("REPL v{}", env!("CARGO_PKG_VERSION")))
        );
        println!(
            "{}",
            style::dim(out, "Type :help for commands, :quit to exit")
        );
    }
    println!();

    let mut input_buffer = String::new();
    loop {
        let prompt = if input_buffer.is_empty() {
            "pleno> "
        } else {
            "... "
        };

        match rl.readline(prompt) {
            Ok(line) => {
                let trimmed = line.trim();

                // Handle REPL commands only on first line
                if input_buffer.is_empty() {
                    match trimmed {
                        ":quit" | ":q" | "exit" | "quit" => {
                            println!("Goodbye!");
                            break;
                        }
                        ":help" | ":h" => {
                            print_help();
                            continue;
                        }
                        ":clear" => {
                            _ = rl.clear_screen(); // rustyline helper
                            vm = VM::new();
                            pleno_stdlib::register(&mut vm);
                            println!("VM state cleared.");
                            continue;
                        }
                        _ => {}
                    }
                }

                if !input_buffer.is_empty() {
                    input_buffer.push('\n');
                }
                input_buffer.push_str(&line);

                if is_complete(&input_buffer) {
                    let _ = rl.add_history_entry(&input_buffer);
                    execute_line(&mut vm, &input_buffer);
                    input_buffer.clear();
                }
            }
            Err(ReadlineError::Interrupted) => {
                // CTRL+C
                if input_buffer.is_empty() {
                    println!("^C");
                    println!("(To exit, type :quit or press Ctrl+D)");
                } else {
                    println!("^C (Input cancelled)");
                    input_buffer.clear();
                }
                continue;
            }
            Err(ReadlineError::Eof) => {
                // CTRL+D
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }

    Ok(())
}

fn execute_line(vm: &mut VM, line: &str) {
    let source_map = SourceMap::new("repl".to_string(), line.to_string());

    // Lex
    let tokens = match lex(line) {
        Ok(t) => t,
        Err(err) => {
            eprintln!(
                "{}",
                DiagnosticReporter::report(&err.to_diagnostic(), &source_map)
            );
            return;
        }
    };

    // Parse
    let mut parser = pleno_parser::Parser::new(tokens, line);
    let mut program = match parser.parse_program() {
        Ok(p) => p,
        Err(err) => {
            eprintln!(
                "{}",
                DiagnosticReporter::report(&err.to_diagnostic(), &source_map)
            );
            return;
        }
    };

    // Compile
    let mut compiler = Compiler::new(line.to_string());
    let chunk = match compiler.compile(&mut program) {
        Ok(c) => c,
        Err(err) => {
            eprintln!(
                "{}",
                DiagnosticReporter::report(&err.diagnostic, &source_map)
            );
            return;
        }
    };

    // Execute
    match vm.execute(chunk) {
        Ok(value) => {
            // Only print non-null values
            if !matches!(value, pleno_value::Value::Null) {
                println!("=> {}", value);
            }
        }
        Err(err) => {
            let diag = if let Some(span) = err.span {
                Diagnostic::error(&err.message).with_label(span, "")
            } else {
                Diagnostic::error(&err.message)
            };
            eprintln!("{}", DiagnosticReporter::report(&diag, &source_map));
        }
    }
}

fn print_help() {
    println!("REPL Commands:");
    println!("  :help, :h    - Show this help message");
    println!("  :quit, :q    - Exit the REPL");
    println!("  :clear       - Clear VM state");
    println!("  exit, quit   - Exit the REPL");
    println!();
    println!("Keyboard shortcuts:");
    println!("  CTRL+C       - Cancel current input");
    println!("  CTRL+D       - Exit the REPL");
    println!("  Up/Down      - Navigate command history");
    println!("  Left/Right   - Move cursor");
    println!("  Home/End     - Jump to start/end of line");
}

fn dirs_history_path() -> Option<String> {
    // Try HOME first (Linux/macOS)
    if let Ok(home) = std::env::var("HOME") {
        return Some(format!("{}/.pleno_history", home));
    }

    // Try USERPROFILE (Windows)
    if let Ok(home) = std::env::var("USERPROFILE") {
        return Some(format!("{}/.pleno_history", home));
    }

    // Fallback to current directory
    match std::env::current_dir() {
        Ok(path) => Some(format!("{}/.pleno_history", path.display())),
        Err(_) => Some(".pleno_history".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_complete() {
        assert!(is_complete("let x = 1;"));
        assert!(!is_complete("let x = ["));
        assert!(!is_complete("let x = [\n1,"));
        assert!(is_complete("let x = [\n1,\n2\n];"));
        assert!(!is_complete("fn test() {"));
        assert!(is_complete("fn test() {\n}"));
        assert!(!is_complete("/* multi-line comment"));
        assert!(is_complete("/* multi-line comment */"));
        assert!(!is_complete("\"unterminated string"));
        assert!(is_complete("\"terminated string\""));
    }
}
