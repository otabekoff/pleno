use pleno_compiler::Compiler;
use pleno_lexer::lex;
use pleno_vm::VM;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, Result};

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

// ... imports ...

pub fn start() -> Result<()> {
    let mut vm = VM::new();
    pleno_stdlib::register(&mut vm);

    let config = rustyline::Config::builder()
        .edit_mode(rustyline::EditMode::Emacs)
        .build();
    let mut rl = DefaultEditor::with_config(config)?;

    // Load history if exists
    let history_path = dirs_history_path();
    if let Some(ref path) = history_path {
        if let Err(_e) = rl.load_history(path) {
            // It's okay if history doesn't exist yet, but warn on other errors
            // println!("Warning: Could not load history: {}", e);
        }
    }

    println!("Pleno REPL v0.1.0");
    println!("Type :help for commands, :quit to exit");
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
    // Lex
    let tokens = match pleno_lexer::lex(line) {
        Ok(t) => t,
        Err(err) => {
            eprintln!(
                "lex error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            return;
        }
    };

    // Parse
    let mut parser = pleno_parser::Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(err) => {
            eprintln!(
                "parse error at {}..{}: {}",
                err.span.start, err.span.end, err.message
            );
            return;
        }
    };

    // Compile
    let mut compiler = Compiler::new(line.to_string());
    let chunk = match compiler.compile(&program) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("compile error: {}", err.message);
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
            eprintln!("runtime error: {}", err.message);
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
